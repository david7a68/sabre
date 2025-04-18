use std::cell::RefCell;
use std::fs::File;
use std::hash::Hash;
use std::io::Cursor;
use std::path::Path;
use std::rc::Rc;

use image::ImageDecoder;
use image::ImageReader;

use super::uploader::Uploader;

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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
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
}

impl TextureManager {
    pub fn new(device: wgpu::Device) -> Self {
        let inner = TextureManagerInner {
            device,
            staging: wgpu::util::StagingBelt::new(1024),
            textures: Vec::new(),
            free_slots: Vec::new(),
        };

        Self {
            inner: Rc::new(RefCell::new(inner)),
        }
    }

    pub fn reserve(&self, path: impl AsRef<Path>) -> Result<Texture, TextureLoadError> {
        let mut inner = self.inner.borrow_mut();

        let path = path.as_ref();

        let file = File::open(path)?;
        let mapping = unsafe { memmap2::Mmap::map(&file) }?;

        let reader = ImageReader::new(Cursor::new(&mapping)).with_guessed_format()?;
        let decoder = reader.into_decoder()?;
        let (width, height) = decoder.dimensions();

        // let texture = inner.device.create_texture(&wgpu::TextureDescriptor {
        //     label: Some(path.to_string_lossy().as_ref()),
        //     size: wgpu::Extent3d {
        //         width,
        //         height,
        //         depth_or_array_layers: 1,
        //     },
        //     mip_level_count: 1,
        //     sample_count: 1,
        //     dimension: wgpu::TextureDimension::D2,
        //     format: wgpu::TextureFormat::Rgba8UnormSrgb,
        //     usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        //     view_formats: &[],
        // });

        let (index, version) = if let Some(free_slot_idx) = inner.free_slots.pop() {
            let slot = &mut inner.textures[free_slot_idx as usize];
            slot.version += 1;
            slot.texture = None;
            (free_slot_idx, slot.version)
        } else {
            let index = inner.textures.len() as u32;
            inner.textures.push(TextureSlot {
                version: 0,
                refcounts: 1,
                texture: None,
            });
            (index, 0)
        };

        Ok(Texture {
            index,
            version,
            width: width as u16,
            height: height as u16,
            manager: self.inner.clone(),
        })
    }
}

struct TextureManagerInner {
    device: wgpu::Device,
    staging: wgpu::util::StagingBelt,
    textures: Vec<TextureSlot>,
    free_slots: Vec<u32>,
    // loaded_textures: Vec<
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

struct TextureSlot {
    version: u32,
    refcounts: u32,
    texture: Option<wgpu::Texture>,
}

struct UploadBuffer {
    buffer: wgpu::Buffer,
    offset: u64,
}
