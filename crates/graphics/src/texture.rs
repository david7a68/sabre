use std::cell::Cell;
use std::cell::RefCell;
use std::fs::File;
use std::io::Cursor;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc;

use guillotiere::AllocId;
use guillotiere::Allocation;
use guillotiere::AtlasAllocator;
use guillotiere::euclid::default::Box2D;
use guillotiere::size2;
use image::ImageDecoder;
use image::ImageReader;
use slotmap::SlotMap;
use slotmap::new_key_type;
use tracing::debug;
use tracing::field::Empty;
use tracing::info;
use tracing::info_span;
use tracing::instrument;
use tracing::warn;

new_key_type! {
    pub struct TextureId;

    struct RawStorageId;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StorageId {
    id: RawStorageId,
    format: wgpu::TextureFormat,
}

impl Default for StorageId {
    fn default() -> Self {
        // The format we choose here is not important because the
        // `RawStorageId::default()` means that the `StorageId` will never be
        // valid anyway.
        Self {
            id: RawStorageId::default(),
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
        }
    }
}

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

pub struct Texture {
    id: TextureId,
    storage_id: StorageId,
    uvwh: [f32; 4],

    view: wgpu::TextureView,
    manager: Rc<TextureManagerInner>,
}

impl Texture {
    #[must_use]
    pub fn id(&self) -> TextureId {
        self.id
    }

    pub(crate) fn storage_id(&self) -> StorageId {
        self.storage_id
    }

    #[must_use]
    pub fn format(&self) -> wgpu::TextureFormat {
        self.storage_id.format
    }

    #[must_use]
    pub fn uvwh(&self) -> [f32; 4] {
        self.uvwh
    }

    #[must_use]
    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.view
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.manager
            .inspect(self.id, |usage| usage.is_ready)
            .unwrap()
    }
}

impl Clone for Texture {
    fn clone(&self) -> Self {
        self.manager.get(self.id).unwrap()
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        self.manager.release(self.id);
    }
}

impl std::fmt::Debug for Texture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Texture")
            .field("id", &self.id)
            .field("storage_id", &self.storage_id.id)
            .field("uvwh", &self.uvwh)
            .field("format", &self.storage_id.format)
            .finish()
    }
}

#[derive(Clone)]
pub struct TextureManager {
    inner: Rc<TextureManagerInner>,

    white_pixel: Texture,
    opaque_pixel: Texture,
}

impl TextureManager {
    pub fn new(queue: wgpu::Queue, device: wgpu::Device) -> Self {
        let inner = TextureManagerInner::new(queue, device);

        let white_pixel = inner.white_pixel();
        let opaque_pixel = inner.opaque_pixel();

        Self {
            inner,
            white_pixel,
            opaque_pixel,
        }
    }

    pub fn white_pixel(&self) -> &Texture {
        &self.white_pixel
    }

    pub fn opaque_pixel(&self) -> &Texture {
        &self.opaque_pixel
    }

    #[instrument(skip(self, data))]
    pub fn from_memory(&self, data: &[u8], width: u16, format: wgpu::TextureFormat) -> Texture {
        self.inner.from_memory(data, width, format)
    }

    #[instrument(skip(self), fields(path = %path.as_ref().display()))]
    pub fn load(&self, path: impl AsRef<Path>) -> Result<Texture, TextureLoadError> {
        self.inner.load(path)
    }

    pub fn flush(&self) {
        self.inner.flush();
    }

    pub fn end_frame(&self) {
        self.inner.end_frame();
    }
}

struct TextureManagerInner {
    white_pixel: Cell<TextureId>,
    opaque_pixel: Cell<TextureId>,

    texture_map: RefCell<SlotMap<TextureId, TextureUsage>>,
    rgba_textures: RefCell<FormattedTextureManager>,
    srgba_textures: RefCell<FormattedTextureManager>,
    alpha_textures: RefCell<FormattedTextureManager>,

    queue: wgpu::Queue,
    device: wgpu::Device,

    ready_sender: mpsc::Sender<TextureId>,
    ready_receiver: mpsc::Receiver<TextureId>,
}

impl TextureManagerInner {
    fn new(queue: wgpu::Queue, device: wgpu::Device) -> Rc<Self> {
        let rgba_textures = FormattedTextureManager {
            format: wgpu::TextureFormat::Rgba8Unorm,
            storage: SlotMap::with_key(),
        };

        let srgba_textures = FormattedTextureManager {
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            storage: SlotMap::with_key(),
        };

        let alpha_textures = FormattedTextureManager {
            format: wgpu::TextureFormat::R8Unorm,
            storage: SlotMap::with_key(),
        };

        let (ready_sender, ready_receiver) = mpsc::channel();

        let this = Rc::new(TextureManagerInner {
            white_pixel: Cell::new(TextureId::default()),
            opaque_pixel: Cell::new(TextureId::default()),
            texture_map: RefCell::new(SlotMap::with_key()),
            rgba_textures: RefCell::new(rgba_textures),
            srgba_textures: RefCell::new(srgba_textures),
            alpha_textures: RefCell::new(alpha_textures),
            queue,
            device,
            ready_sender,
            ready_receiver,
        });

        // Set up the white pixel and forget it so that its refcount is never 0.
        let white_pixel =
            this.from_memory(&[255, 255, 255, 255], 1, wgpu::TextureFormat::Rgba8Unorm);

        this.white_pixel.set(white_pixel.id());
        std::mem::forget(white_pixel);

        // Set up the opaque pixel and forget it so that its refcount is never 0.
        let opaque_pixel = this.from_memory(&[255], 1, wgpu::TextureFormat::R8Unorm);

        this.opaque_pixel.set(opaque_pixel.id());
        std::mem::forget(opaque_pixel);

        this
    }

    fn white_pixel(self: &Rc<Self>) -> Texture {
        self.get(self.white_pixel.get()).unwrap()
    }

    fn opaque_pixel(self: &Rc<Self>) -> Texture {
        self.get(self.opaque_pixel.get()).unwrap()
    }

    fn get(self: &Rc<Self>, id: TextureId) -> Option<Texture> {
        let mut texture_map = self.texture_map.borrow_mut();

        let usage = texture_map.get_mut(id)?;
        usage.refcount += 1;

        Some(Texture {
            id,
            storage_id: usage.storage,
            uvwh: usage.uvwh,
            view: usage.view.clone(),
            manager: self.clone(),
        })
    }

    fn release(self: &Rc<Self>, id: TextureId) {
        let mut texture_map = self.texture_map.borrow_mut();

        if let Some(usage) = texture_map.get_mut(id) {
            usage.refcount -= 1;

            if usage.refcount == 0 {
                debug!(
                    ?id,
                    "All texture references released, freeing texture storage"
                );

                let usage = texture_map.remove(id).unwrap();

                let storage = match usage.format {
                    wgpu::TextureFormat::Rgba8Unorm => &self.rgba_textures,
                    wgpu::TextureFormat::Rgba8UnormSrgb => &self.srgba_textures,
                    wgpu::TextureFormat::R8Unorm => &self.alpha_textures,
                    _ => unreachable!(),
                };

                storage
                    .borrow_mut()
                    .release(usage.storage.id, usage.atlas_id);
            }
        }
    }

    #[instrument(skip(self), fields(texture_id = ?id))]
    fn release_mut(&mut self, id: TextureId) {
        let texture_map = self.texture_map.get_mut();

        if let Some(usage) = texture_map.get_mut(id) {
            usage.refcount -= 1;
            if usage.refcount == 0 {
                let usage = texture_map.remove(id).unwrap();

                let storage = match usage.format {
                    wgpu::TextureFormat::Rgba8Unorm => &self.rgba_textures,
                    wgpu::TextureFormat::Rgba8UnormSrgb => &self.srgba_textures,
                    wgpu::TextureFormat::R8Unorm => &self.alpha_textures,
                    _ => unreachable!(),
                };

                storage
                    .borrow_mut()
                    .release(usage.storage.id, usage.atlas_id);
            }
        }
    }

    pub fn inspect<T>(
        self: &Rc<Self>,
        texture: TextureId,
        callback: impl Fn(&TextureUsage) -> T,
    ) -> Option<T> {
        let texture_map = self.texture_map.borrow();
        let usage = texture_map.get(texture)?;
        Some(callback(usage))
    }

    pub fn from_memory(
        self: &Rc<Self>,
        data: &[u8],
        width: u16,
        format: wgpu::TextureFormat,
    ) -> Texture {
        let bytes_per_row = width as usize * bytes_per_pixel(format);
        let height = (data.len() / bytes_per_row)
            .try_into()
            .expect("Max texture dimension of 65535 exceeded.");

        assert!(
            data.len() % bytes_per_row == 0,
            "Data length is not a multiple of width and pixel size: data.len() = {}, width = {}, bytes per pixel = {}",
            data.len(),
            width,
            bytes_per_pixel(format)
        );

        let mut manager = match format {
            wgpu::TextureFormat::Rgba8UnormSrgb => &self.srgba_textures,
            wgpu::TextureFormat::Rgba8Unorm => &self.rgba_textures,
            wgpu::TextureFormat::R8Unorm => &self.alpha_textures,
            _ => unreachable!(),
        }
        .borrow_mut();

        let (texture, usage, rectangle) = manager.allocate(width, height, &self.device);

        let uvwh = usage.uvwh;
        let view = usage.view.clone();
        let storage_id = usage.storage;
        let texture_id = self.texture_map.borrow_mut().insert(usage);

        info!(
            x = rectangle.x_range().start,
            y = rectangle.y_range().start,
            width = rectangle.width(),
            height = rectangle.height(),
            uvwh = ?uvwh,
            texture_id = ?texture_id,
            bytes_per_pixel = bytes_per_pixel(format),
            "Loaded texture from memory"
        );

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: rectangle.x_range().start.try_into().unwrap(),
                    y: rectangle.y_range().start.try_into().unwrap(),
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row as u32),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: width.into(),
                height: height.into(),
                depth_or_array_layers: 1,
            },
        );

        self.ready_sender.send(texture_id).unwrap();

        Texture {
            id: texture_id,
            storage_id,
            view,
            uvwh,
            manager: self.clone(),
        }
    }

    fn load(self: &Rc<Self>, path: impl AsRef<Path>) -> Result<Texture, TextureLoadError> {
        let start_time = std::time::Instant::now();

        let path = path.as_ref();

        let file = File::open(path)?;
        let mapping = unsafe { memmap2::Mmap::map(&file) }?;

        let ((width, height), color_type, bytes_per_pixel) = {
            let reader = ImageReader::new(Cursor::new(&mapping)).with_guessed_format()?;
            let decoder = reader.into_decoder()?;
            let color_type = decoder.color_type();
            (
                decoder.dimensions(),
                color_type,
                color_type.bytes_per_pixel(),
            )
        };

        let (format, mut manager) = match color_type {
            image::ColorType::Rgba8 => (
                wgpu::TextureFormat::Rgba8UnormSrgb,
                self.srgba_textures.borrow_mut(),
            ),
            other => unimplemented!("Unsupported color type: {:?}", other),
        };

        let (texture, usage, rectangle) = manager.allocate(
            width.try_into().unwrap(),
            height.try_into().unwrap(),
            &self.device,
        );

        let uvwh = usage.uvwh;
        let view = usage.view.clone();
        let storage_id = usage.storage;
        let texture_id = self.texture_map.borrow_mut().insert(usage);

        tokio::task::spawn_blocking({
            let span = info_span!(
                "Loading texture from file",
                path = %path.display(),
                texture_id = debug(texture_id),
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
                        origin: wgpu::Origin3d {
                            x: rectangle.x_range().start.try_into().unwrap(),
                            y: rectangle.y_range().start.try_into().unwrap(),
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &temp,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(width * u32::from(bytes_per_pixel)),
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );

                ready.send(texture_id).unwrap();

                info!(
                    x = rectangle.x_range().start,
                    y = rectangle.y_range().start,
                    uvwh = ?uvwh,
                    texture_id = ?texture_id,
                    load_time = ?start_time.elapsed(),
                    format = ?format,
                    "Loaded texture from file"
                );
            }
        });

        Ok(Texture {
            id: texture_id,
            storage_id,
            view,
            uvwh,
            manager: self.clone(),
        })
    }

    fn flush(self: &Rc<Self>) {
        while let Ok(texture_id) = self.ready_receiver.try_recv() {
            if let Some(usage) = self.texture_map.borrow_mut().get_mut(texture_id) {
                usage.is_ready = true;
            }
        }
    }

    fn end_frame(self: &Rc<Self>) {
        self.rgba_textures.borrow_mut().end_frame();
        self.srgba_textures.borrow_mut().end_frame();
        self.alpha_textures.borrow_mut().end_frame();
    }
}

impl Drop for TextureManagerInner {
    fn drop(&mut self) {
        debug!("Dropping texture manager");
        self.release_mut(self.white_pixel.take());
        self.release_mut(self.opaque_pixel.take());
    }
}

#[derive(Clone)]
struct TextureUsage {
    storage: StorageId,
    is_ready: bool,
    refcount: u32,
    atlas_id: AllocId,
    format: wgpu::TextureFormat,
    uvwh: [f32; 4],
    view: wgpu::TextureView,
}

#[derive(Clone)]
struct TextureStorage {
    refcount: u32,
    atlas: AtlasAllocator,
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
}

impl Drop for TextureStorage {
    fn drop(&mut self) {
        info!(refcount = self.refcount, texture = ?self.texture, "Releasing texture storage");

        self.texture.destroy();
    }
}

struct FormattedTextureManager {
    format: wgpu::TextureFormat,
    storage: SlotMap<RawStorageId, TextureStorage>,
}

impl Drop for FormattedTextureManager {
    fn drop(&mut self) {
        info!(format = ?self.format, "Dropping texture manager");

        self.end_frame();

        for storage in self.storage.values() {
            if storage.refcount > 0 {
                warn!(
                    refcount = storage.refcount,
                    "Texture storage not released before drop"
                );
            }
        }
    }
}

impl FormattedTextureManager {
    /// Call once per frame to clean up resources and perform any necessary
    /// housekeeping.
    fn end_frame(&mut self) {
        self.storage.retain(|id, storage| {
            if storage.refcount == 0 {
                warn!(storage = ?id, format = ?self.format, "Dropping texture atlas storage");
                storage.texture.destroy();
                return false;
            }

            storage.refcount > 0
        });
    }

    /// Release a reference for the given texture.
    fn release(&mut self, storage_id: RawStorageId, node: AllocId) {
        let storage = self.storage.get_mut(storage_id).unwrap();
        storage.atlas.deallocate(node);
        storage.refcount -= 1;
    }

    #[instrument(skip(self, device))]
    fn allocate(
        &mut self,
        width: u16,
        height: u16,
        device: &wgpu::Device,
    ) -> (wgpu::Texture, TextureUsage, Box2D<i32>) {
        let alloc_size = size2(width.into(), height.into());

        let (storage_id, texture, view, atlas_rect, Allocation { id, rectangle }) = 'alloc: {
            for (storage_id, storage) in &mut self.storage {
                if let Some(allocation) = storage.atlas.allocate(alloc_size) {
                    storage.refcount += 1;
                    break 'alloc (
                        storage_id,
                        storage.texture.clone(),
                        storage.texture_view.clone(),
                        storage.atlas.size(),
                        allocation,
                    );
                }
            }

            // If we reach here, we need to allocate a new texture storage.
            let atlas_width = 4096.max(width);
            let atlas_height = 4096.max(height);

            let label = match self.format {
                wgpu::TextureFormat::Rgba8UnormSrgb => "Atlas Texture (sRGB)",
                wgpu::TextureFormat::Rgba8Unorm => "Atlas Texture (RGBA)",
                wgpu::TextureFormat::R8Unorm => "Atlas Texture (Alpha)",
                _ => unreachable!(),
            };

            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: atlas_width.into(),
                    height: atlas_height.into(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

            let atlas_size = size2(atlas_width.into(), atlas_height.into());

            let mut storage = TextureStorage {
                refcount: 1,
                atlas: AtlasAllocator::new(atlas_size),
                texture: texture.clone(),
                texture_view: texture_view.clone(),
            };

            let allocation = storage.atlas.allocate(alloc_size).unwrap();
            let storage_id = self.storage.insert(storage);

            debug!(
                storage_id = ?storage_id,
                width = atlas_width,
                height = atlas_height,
                format = ?self.format,
                "Allocated new texture storage"
            );

            (storage_id, texture, texture_view, atlas_size, allocation)
        };

        // Inset the rectangle by 0.5 pixels to avoid sampling bleed.
        let uv_rect = rectangle.cast::<f32>().inflate(-0.5, -0.5);
        let u = uv_rect.x_range().start / atlas_rect.width as f32;
        let v = uv_rect.y_range().start / atlas_rect.height as f32;
        let w = uv_rect.width() / atlas_rect.width as f32;
        let h = uv_rect.height() / atlas_rect.height as f32;

        (
            texture,
            TextureUsage {
                storage: StorageId {
                    id: storage_id,
                    format: self.format,
                },
                is_ready: false,
                refcount: 1,
                atlas_id: id,
                format: self.format,
                view,
                uvwh: [u, v, w, h],
            },
            rectangle,
        )
    }
}

fn bytes_per_pixel(format: wgpu::TextureFormat) -> usize {
    match format {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => 4,
        wgpu::TextureFormat::R8Unorm => 1,
        _ => unimplemented!(),
    }
}
