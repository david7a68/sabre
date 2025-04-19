use std::cell::RefCell;
use std::fs::File;
use std::hash::Hash;
use std::io::Cursor;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc;

use image::ImageDecoder;
use image::ImageReader;
use tracing::field::Empty;
use tracing::info;
use tracing::info_span;

#[derive(Debug)]
pub enum TextureLoadError {
    Decoding(Box<dyn std::error::Error>),
    Io(std::io::Error),
}

impl From<std::io::Error> for TextureLoadError {
    fn from(err: std::io::Error) -> Self {
        TextureLoadError::Io(err)
    }
}

impl From<image::ImageError> for TextureLoadError {
    fn from(err: image::ImageError) -> Self {
        match err {
            image::ImageError::IoError(err) => TextureLoadError::Io(err),
            other => TextureLoadError::Decoding(Box::new(other)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureId {
    pub index: u32,
    pub version: u32,
}

#[derive(Clone)]
pub struct Texture {
    index: u32,
    version: u32,

    pub width: u16,
    pub height: u16,

    manager: Rc<RefCell<TextureManagerInner>>,
}

impl Texture {
    pub fn id(&self) -> TextureId {
        TextureId {
            index: self.index,
            version: self.version,
        }
    }
}

impl PartialEq for Texture {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.version == other.version
    }
}

impl Eq for Texture {}

impl Hash for Texture {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.version.hash(state);
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        let mut inner = self.manager.borrow_mut();
        inner.release(self.index);
    }
}

#[derive(Clone)]
pub(crate) struct TextureManager {
    inner: Rc<RefCell<TextureManagerInner>>,

    queue: wgpu::Queue,
    device: wgpu::Device,

    ready_sender: mpsc::Sender<TextureId>,
}

impl TextureManager {
    pub fn new(queue: wgpu::Queue, device: wgpu::Device) -> Self {
        let (ready_sender, ready_receiver) = mpsc::channel();

        let inner = TextureManagerInner {
            textures: Vec::new(),
            free_slots: Vec::new(),
            ready_receiver,
        };

        Self {
            queue,
            device,
            inner: Rc::new(RefCell::new(inner)),
            ready_sender,
        }
    }

    pub fn load(&self, path: impl AsRef<Path>) -> Result<Texture, TextureLoadError> {
        let start_time = std::time::Instant::now();

        let mut inner = self.inner.borrow_mut();

        let path = path.as_ref();

        let file = File::open(path)?;
        let mapping = unsafe { memmap2::Mmap::map(&file) }?;

        let ((width, height), bytes_per_pixel) = {
            let reader = ImageReader::new(Cursor::new(&mapping)).with_guessed_format()?;
            let decoder = reader.into_decoder()?;
            (decoder.dimensions(), decoder.color_type().bytes_per_pixel())
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(path.to_string_lossy().as_ref()),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let (index, version, slot) = if let Some(free_slot_idx) = inner.free_slots.pop() {
            let slot = &mut inner.textures[free_slot_idx as usize];
            (free_slot_idx, slot.version, slot)
        } else {
            let index = inner.textures.len() as u32;
            inner.textures.push(TextureSlot::default());
            (index, 0, inner.textures.last_mut().unwrap())
        };

        // 1 reference. We don't care about the loader thread because the
        // texture itself is also refcounted by wgpu.
        slot.refcounts = 1;
        slot.texture = Some(texture.clone());

        tokio::task::spawn_blocking({
            let span = info_span!(
                "Texture load",
                path = %path.display(),
                texture_id = debug(TextureId { index, version}),
                width = width,
                height = height,
                file_size = mapping.len(),
                decoded_size = Empty,
            );

            let queue = self.queue.clone();
            let ready = self.ready_sender.clone();

            move || {
                let _enter = span.enter();

                let temp = {
                    let reader = ImageReader::new(Cursor::new(&mapping))
                        .with_guessed_format()
                        .unwrap();

                    let decoder = reader.into_decoder().unwrap();
                    let mut temp = vec![0; decoder.total_bytes() as usize];
                    span.record("decoded_size", temp.len());

                    decoder.read_image(&mut temp).unwrap();

                    temp
                };

                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &temp,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(width * bytes_per_pixel as u32),
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );

                ready.send(TextureId { index, version }).unwrap();

                info!(
                    "Texture loaded after {} ms",
                    start_time.elapsed().as_secs_f32() * 1000.0
                );
            }
        });

        Ok(Texture {
            index,
            version,
            width: width as u16,
            height: height as u16,
            manager: self.inner.clone(),
        })
    }

    /// Flushes the texture manager, updating the state of any textures that
    /// have been loaded since the last time this method was called.
    pub fn flush(&mut self) {
        let mut inner = self.inner.borrow_mut();

        while let Ok(texture_id) = inner.ready_receiver.try_recv() {
            let slot = &mut inner.textures[texture_id.index as usize];

            if slot.version == texture_id.version {
                slot.is_copied = true;
            }
        }
    }
}

struct TextureManagerInner {
    textures: Vec<TextureSlot>,
    free_slots: Vec<u32>,
    ready_receiver: mpsc::Receiver<TextureId>,
}

impl TextureManagerInner {
    fn release(&mut self, slot_index: u32) {
        let slot = &mut self.textures[slot_index as usize];
        slot.refcounts -= 1;

        if slot.refcounts == 0 {
            slot.version += 1;
            slot.texture = None;
            self.free_slots.push(slot_index);
        }
    }
}

#[derive(Default)]
struct TextureSlot {
    version: u32,
    texture: Option<wgpu::Texture>,
    is_copied: bool,
    refcounts: u32,
}
