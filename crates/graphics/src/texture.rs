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

new_key_type! {
    pub struct TextureId;

    struct StorageId;
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
    uvwh: [f32; 4],
    format: wgpu::TextureFormat,

    manager: Rc<TextureManagerInner>,
}

impl Texture {
    #[must_use]
    pub fn id(&self) -> TextureId {
        self.id
    }

    #[must_use]
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    #[must_use]
    pub fn uvwh(&self) -> [f32; 4] {
        self.uvwh
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.manager.inspect(|usage| usage.is_ready).unwrap()
    }

    #[must_use]
    pub fn storage(&self) -> TextureStorage {
        self.manager.get_storage(self.id).unwrap()
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
            .field("uvwh", &self.uvwh)
            .field("format", &self.format)
            .finish()
    }
}

#[derive(Clone)]
pub struct TextureStorage {
    refcount: u32,
    atlas: AtlasAllocator,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    texture_view: wgpu::TextureView,
}

impl TextureStorage {
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.texture_view
    }
}

pub struct TextureManager {
    inner: Rc<TextureManagerInner>,
}

impl TextureManager {
    pub fn new(
        queue: wgpu::Queue,
        device: wgpu::Device,
        texture_bind_group_layout: wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            inner: TextureManagerInner::new(queue, device, texture_bind_group_layout),
        }
    }

    pub fn white_pixel(&self) -> Texture {
        self.inner.white_pixel()
    }

    #[instrument(skip(self, data))]
    pub fn from_memory(
        &self,
        data: &[u8],
        width: u16,
        format: wgpu::TextureFormat,
        name: Option<&str>,
    ) -> Texture {
        self.inner.from_memory(data, width, format, name)
    }

    #[instrument(skip(self), fields(path = %path.as_ref().display()))]
    pub fn load(&mut self, path: impl AsRef<Path>) -> Result<Texture, TextureLoadError> {
        self.inner.load(path)
    }

    pub fn flush(&mut self) {
        self.inner.flush();
    }

    pub fn end_frame(&mut self) {
        self.inner.end_frame();
    }
}

struct TextureManagerInner {
    white_pixel: Cell<TextureId>,
    texture_map: RefCell<SlotMap<TextureId, TextureUsage>>,
    rgba_textures: RefCell<FormattedTextureManager>,
    srgba_textures: RefCell<FormattedTextureManager>,
    alpha_textures: RefCell<FormattedTextureManager>,

    queue: wgpu::Queue,
    device: wgpu::Device,

    ready_sender: mpsc::Sender<TextureId>,
    ready_receiver: mpsc::Receiver<TextureId>,

    /// The bind group layout that we use for textures. Storing it here allows
    /// us to construct bind groups for our textures once.
    texture_bind_group_layout: wgpu::BindGroupLayout,
}

impl TextureManagerInner {
    fn new(
        queue: wgpu::Queue,
        device: wgpu::Device,
        texture_bind_group_layout: wgpu::BindGroupLayout,
    ) -> Rc<Self> {
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
            texture_map: RefCell::new(SlotMap::with_key()),
            rgba_textures: RefCell::new(rgba_textures),
            srgba_textures: RefCell::new(srgba_textures),
            alpha_textures: RefCell::new(alpha_textures),
            queue,
            device,
            ready_sender,
            ready_receiver,
            texture_bind_group_layout,
        });

        // Set up the white pixel (technically 2x2 to avoid bleed) and forget it
        // so that its refcount is never 0.
        let white_pixel = this.from_memory(
            &[255, 255, 255, 255],
            1,
            wgpu::TextureFormat::Rgba8Unorm,
            Some("White Pixel"),
        );

        this.white_pixel.set(white_pixel.id());
        std::mem::forget(white_pixel);

        this
    }

    fn white_pixel(self: &Rc<Self>) -> Texture {
        self.get(self.white_pixel.get()).unwrap()
    }

    fn get(self: &Rc<Self>, id: TextureId) -> Option<Texture> {
        let mut texture_map = self.texture_map.borrow_mut();

        let usage = texture_map.get_mut(id)?;
        usage.refcount += 1;

        Some(Texture {
            id,
            uvwh: usage.uvwh,
            format: usage.format,
            manager: self.clone(),
        })
    }

    fn release(self: &Rc<Self>, id: TextureId) {
        let mut texture_map = self.texture_map.borrow_mut();

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

                storage.borrow_mut().release(usage.storage, usage.atlas_id);
            }
        }
    }

    fn get_storage(self: &Rc<Self>, id: TextureId) -> Option<TextureStorage> {
        let texture_map = self.texture_map.borrow();

        let usage = texture_map.get(id)?;

        let storage = match usage.format {
            wgpu::TextureFormat::Rgba8Unorm => &self.rgba_textures,
            wgpu::TextureFormat::Rgba8UnormSrgb => &self.srgba_textures,
            wgpu::TextureFormat::R8Unorm => &self.alpha_textures,
            _ => unreachable!(),
        }
        .borrow();

        storage.storage.get(usage.storage).cloned()
    }

    pub fn inspect<T>(self: &Rc<Self>, callback: impl Fn(&TextureUsage) -> T) -> Option<T> {
        let texture_map = self.texture_map.borrow();
        let usage = texture_map.get(self.white_pixel.get())?;
        Some(callback(usage))
    }

    #[instrument(skip(self, data))]
    pub fn from_memory(
        self: &Rc<Self>,
        data: &[u8],
        width: u16,
        format: wgpu::TextureFormat,
        name: Option<&str>,
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

        let (texture, usage, rectangle) =
            manager.allocate(width, height, &self.device, &self.texture_bind_group_layout);

        let uvwh = usage.uvwh;
        let texture_id = self.texture_map.borrow_mut().insert(usage);

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
                rows_per_image: Some(height.into()),
            },
            wgpu::Extent3d {
                width: width.into(),
                height: height.into(),
                depth_or_array_layers: 1,
            },
        );

        self.ready_sender.send(texture_id).unwrap();

        info!(
            x = rectangle.x_range().start,
            y = rectangle.y_range().start,
            width = rectangle.width(),
            height = rectangle.height(),
            uvwh = ?uvwh,
            texture_id = ?texture_id,
            "Loaded texture from memory"
        );

        Texture {
            id: texture_id,
            uvwh,
            format,
            manager: self.clone(),
        }
    }

    #[instrument(skip(self, path), fields(path = %path.as_ref().display()))]
    fn load(self: &Rc<Self>, path: impl AsRef<Path>) -> Result<Texture, TextureLoadError> {
        info!("Loading texture from file: {:?}", path.as_ref().display());

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

        let mut manager = match color_type {
            image::ColorType::Rgba8 => self.srgba_textures.borrow_mut(),
            other => unimplemented!("Unsupported color type: {:?}", other),
        };

        let (texture, usage, rectangle) = manager.allocate(
            width.try_into().unwrap(),
            height.try_into().unwrap(),
            &self.device,
            &self.texture_bind_group_layout,
        );

        let uvwh = usage.uvwh;
        let texture_id = self.texture_map.borrow_mut().insert(usage);

        tokio::task::spawn_blocking({
            let span = info_span!(
                "Texture load",
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
                    "Loaded texture from file"
                );
            }
        });

        Ok(Texture {
            id: texture_id,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            uvwh,
            manager: self.clone(),
        })
    }

    fn flush(self: &Rc<Self>) {
        while let Ok(texture_id) = self.ready_receiver.try_recv() {
            if let Some(usage) = self.texture_map.borrow_mut().get_mut(texture_id) {
                usage.is_ready = true;
                debug!(texture_id = ?texture_id, "Texture is ready: {texture_id:?}");
            }
        }
    }

    fn end_frame(self: &Rc<Self>) {
        self.rgba_textures.borrow_mut().end_frame();
        self.srgba_textures.borrow_mut().end_frame();
        self.alpha_textures.borrow_mut().end_frame();
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
}

struct FormattedTextureManager {
    format: wgpu::TextureFormat,
    storage: SlotMap<StorageId, TextureStorage>,
}

impl FormattedTextureManager {
    /// Call once per frame to clean up resources and perform any necessary
    /// housekeeping.
    fn end_frame(&mut self) {
        self.storage.retain(|_, storage| storage.refcount > 0);
    }

    /// Release a reference for the given texture.
    fn release(&mut self, storage: StorageId, node: AllocId) {
        let storage = self.storage.get_mut(storage).unwrap();
        storage.atlas.deallocate(node);
        storage.refcount -= 1;
    }

    #[instrument(skip(self, device, bind_group_layout))]
    fn allocate(
        &mut self,
        width: u16,
        height: u16,
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> (wgpu::Texture, TextureUsage, Box2D<i32>) {
        // If we reach here, we need to allocate a new texture storage.
        let atlas_width = 4096.max(width);
        let atlas_height = 4096.max(height);

        let (storage_id, texture, atlas_rect, Allocation { id, rectangle }) = (|| {
            for (storage_id, storage) in &mut self.storage {
                if let Some(allocation) = storage.atlas.allocate(size2(width.into(), height.into()))
                {
                    storage.refcount += 1;
                    return (
                        storage_id,
                        storage.texture.clone(),
                        storage.atlas.size(),
                        allocation,
                    );
                }
            }

            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Atlas Texture"),
                size: wgpu::Extent3d {
                    width: atlas_width.into(),
                    height: atlas_height.into(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                }],
            });

            let atlas_size = size2(atlas_width.into(), atlas_height.into());

            let mut storage = TextureStorage {
                refcount: 1,
                atlas: AtlasAllocator::new(atlas_size),
                texture: texture.clone(),
                bind_group,
                texture_view,
            };

            let allocation = storage
                .atlas
                .allocate(size2(width.into(), height.into()))
                .unwrap();

            let storage_id = self.storage.insert(storage);

            (storage_id, texture, atlas_size, allocation)
        })();

        // Inset the rectangle by 0.5 pixels to avoid sampling bleed.
        let uv_rect = rectangle.cast::<f32>().inflate(-0.5, -0.5);
        let u = uv_rect.x_range().start / atlas_rect.width as f32;
        let v = uv_rect.y_range().start / atlas_rect.height as f32;
        let w = uv_rect.width() / atlas_rect.width as f32;
        let h = uv_rect.height() / atlas_rect.height as f32;

        (
            texture,
            TextureUsage {
                storage: storage_id,
                is_ready: false,
                refcount: 1,
                atlas_id: id,
                format: self.format,
                uvwh: [u, v, w, h],
            },
            rectangle,
        )
    }
}

fn bytes_per_pixel(format: wgpu::TextureFormat) -> usize {
    match format {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => 4,
        _ => unimplemented!(),
    }
}
