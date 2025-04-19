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
use tracing::instrument;

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

/// A unique identifier for a texture.
///
/// `TextureId::default()` points to the white pixel texture used if a texture
/// is not otherwise needed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextureId {
    pub index: u32,
    pub version: u32,
}

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

    pub fn get(&self) -> Option<StoredTexture> {
        let inner = self.manager.borrow();
        let slot = &inner.textures[self.index as usize];

        if slot.version == self.version {
            slot.texture.clone()
        } else {
            None
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

impl Clone for Texture {
    fn clone(&self) -> Self {
        let mut inner = self.manager.borrow_mut();
        let slot = &mut inner.textures[self.index as usize];
        slot.refcounts += 1;

        info!("Cloning texture handle. Refcount: {}", slot.refcounts);

        Texture {
            manager: self.manager.clone(),
            ..*self
        }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        let mut inner = self.manager.borrow_mut();

        inner.release(self.index);
    }
}

#[derive(Clone)]
pub struct StoredTexture {
    pub texture: wgpu::Texture,
    pub is_ready: bool,
    pub view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
}

#[derive(Clone)]
pub(crate) struct TextureManager {
    inner: Rc<RefCell<TextureManagerInner>>,

    queue: wgpu::Queue,
    device: wgpu::Device,

    ready_sender: mpsc::Sender<TextureId>,

    /// The bind group layout that we use for textures. Storing it here allows
    /// us to construct bind groups for our textures once.
    texture_bind_group_layout: wgpu::BindGroupLayout,
}

impl TextureManager {
    pub fn new(
        queue: wgpu::Queue,
        device: wgpu::Device,
        texture_bind_group_layout: wgpu::BindGroupLayout,
    ) -> Self {
        let (ready_sender, ready_receiver) = mpsc::channel();

        let inner = TextureManagerInner {
            textures: Vec::new(),
            free_slots: Vec::new(),
            ready_receiver,
        };

        let this = Self {
            queue,
            device,
            inner: Rc::new(RefCell::new(inner)),
            ready_sender,
            texture_bind_group_layout,
        };

        let white_pixel = this.from_memory(
            &[255, 255, 255, 255],
            1,
            wgpu::TextureFormat::Rgba8Unorm,
            Some("White Pixel"),
        );

        // Always keep a reference to the white pixel texture.
        std::mem::forget(white_pixel);

        this
    }

    pub fn white_pixel(&self) -> Texture {
        let mut inner = self.inner.borrow_mut();

        let slot = &mut inner.textures[0];
        slot.refcounts += 1;

        Texture {
            index: 0,
            version: 0,
            width: 1,
            height: 1,
            manager: self.inner.clone(),
        }
    }

    #[instrument(skip(self, data))]
    pub fn from_memory(
        &self,
        data: &[u8],
        width: u32,
        format: wgpu::TextureFormat,
        name: Option<&str>,
    ) -> Texture {
        let mut inner = self.inner.borrow_mut();

        let bytes_per_row = width * bytes_per_pixel(format);
        let height = data.len() as u32 / bytes_per_row;

        assert!(
            data.len() as u32 % bytes_per_row == 0,
            "Data length is not a multiple of width and pixel size: data.len() = {}, width = {}, bytes per pixel = {}",
            data.len(),
            width,
            bytes_per_pixel(format)
        );

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: name,
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
        slot.texture = {
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.texture_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                }],
            });

            Some(StoredTexture {
                is_ready: false,
                texture: texture.clone(),
                view,
                bind_group,
            })
        };

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.ready_sender
            .send(TextureId { index, version })
            .unwrap();

        Texture {
            index,
            version,
            width: width as u16,
            height: height as u16,
            manager: self.inner.clone(),
        }
    }

    #[instrument(skip(self), fields(path = %path.as_ref().display()))]
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
        slot.texture = {
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.texture_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                }],
            });

            Some(StoredTexture {
                is_ready: false,
                texture: texture.clone(),
                view,
                bind_group,
            })
        };

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
                slot.texture.as_mut().unwrap().is_ready = true;
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

        info!("Dropping texture handle. Refcount: {}", slot.refcounts);

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
    texture: Option<StoredTexture>,
    refcounts: u32,
}

fn bytes_per_pixel(format: wgpu::TextureFormat) -> u32 {
    match format {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => 4,
        _ => unimplemented!(),
    }
}
