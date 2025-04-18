use tokio::sync::mpsc;
use tracing::warn;

pub struct Chunk {
    buffer: Option<Buffer>,
    sender: mpsc::UnboundedSender<Buffer>,
}

impl Drop for Chunk {
    fn drop(&mut self) {
        if self.sender.send(self.buffer.take().unwrap()).is_err() {
            warn!("Failed to send chunk back to uploader. Was it destroyed?");
        }
    }
}

pub struct Uploader {
    device: wgpu::Device,
    chunk_size: u64,

    /// Chunks that are currently accumulating data to be uploaded.
    active_chunks: Vec<Buffer>,
    /// Chunks that are currently being uploaded to the GPU.
    closed_chunks: Vec<Buffer>,
    /// Chunks that are not currently in use and are empty.
    unused_chunks: Vec<Buffer>,

    /// The receiver for chunks that are currently being written to
    /// asynchronously.
    issue_sender: mpsc::UnboundedSender<Buffer>,
    /// The sender for chunks that are currently being written to
    /// asynchronously.
    issue_receiver: mpsc::UnboundedReceiver<Buffer>,

    /// The sender for chunks that are mapped again.
    free_sender: mpsc::UnboundedSender<Buffer>,
    /// Free chunks are received here to be returned to `unused_chunks`.
    free_receiver: mpsc::UnboundedReceiver<Buffer>,
}

impl Uploader {
    pub fn new(device: wgpu::Device, chunk_size: wgpu::BufferSize) -> Self {
        let (issue_sender, issue_receiver) = mpsc::unbounded_channel();
        let (free_sender, free_receiver) = mpsc::unbounded_channel();

        Self {
            device,
            chunk_size: chunk_size.get(),
            active_chunks: Vec::new(),
            closed_chunks: Vec::new(),
            unused_chunks: Vec::new(),
            issue_receiver,
            issue_sender,
            free_sender,
            free_receiver,
        }
    }

    pub fn allocate(
        &mut self,
        size: wgpu::BufferSize,
        alignment: wgpu::BufferSize,
    ) -> wgpu::BufferSlice {
        let alignment = alignment.get().max(wgpu::MAP_ALIGNMENT);

        let mut chunk = self.allocate_chunk(size, alignment);

        let offset = chunk.allocate(size, alignment);
        self.active_chunks.push(chunk);

        let chunk = self.active_chunks.last().unwrap();
        chunk.buffer.slice(offset..offset + size.get())
    }

    pub fn allocate_async(&mut self, size: wgpu::BufferSize, alignment: wgpu::BufferSize) -> Chunk {
        let alignment = alignment.get().max(wgpu::MAP_ALIGNMENT);
        let chunk = self.allocate_chunk(size, alignment);

        Chunk {
            buffer: Some(chunk),
            sender: self.issue_sender.clone(),
        }
    }

    pub fn finish(&mut self) {
        self.receive_issued();

        for chunk in self.active_chunks.drain(..) {
            chunk.buffer.unmap();
            self.closed_chunks.push(chunk);
        }
    }

    pub fn reclaim(&mut self) {
        self.receive_free();

        for chunk in self.closed_chunks.drain(..) {
            let sender = self.free_sender.clone();
            chunk
                .buffer
                .clone()
                .slice(..)
                .map_async(wgpu::MapMode::Write, move |_| {
                    let _ = sender.send(chunk);
                });
        }
    }

    fn allocate_chunk(&mut self, size: wgpu::BufferSize, alignment: u64) -> Buffer {
        assert!(alignment.is_power_of_two());

        if let Some(index) = {
            // Try the active chunks first since they're already partially
            // filled. This is the most common case on the main thread.
            self.active_chunks
                .iter()
                .position(|chunk| chunk.can_allocate(size, alignment))
        } {
            self.active_chunks.swap_remove(index)
        } else if let Some(index) = {
            // Try to use active chunks that were shared with other threads.
            // This makes things slower but lets us pack more data into each
            // buffer sooner (fewer but larger buffers to upload).
            let (idx_offset, range) = self.receive_issued();
            range
                .iter()
                .position(|chunk| chunk.can_allocate(size, alignment))
                .map(|i| idx_offset + i)
        } {
            self.active_chunks.swap_remove(index)
        } else if let Some(index) = self
            .unused_chunks
            .iter()
            .position(|chunk| chunk.can_allocate(size, alignment))
        {
            self.unused_chunks.swap_remove(index)
        } else if let Some(index) = {
            let (idx_offset, range) = self.receive_free();
            range
                .iter()
                .position(|chunk| chunk.can_allocate(size, alignment))
                .map(|i| idx_offset + i)
        } {
            self.unused_chunks.swap_remove(index)
        } else {
            Buffer {
                buffer: self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Staging buffer"),
                    size: size.get().max(self.chunk_size),
                    usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: true,
                }),
                offset: 0,
            }
        }
    }

    /// Recall any chunks that were shared with other threads but are now ready
    /// to be allocated again.
    fn receive_issued(&mut self) -> (usize, &[Buffer]) {
        let i = self.active_chunks.len();

        while let Ok(mut chunk) = self.issue_receiver.try_recv() {
            chunk.offset = 0;
            self.active_chunks.push(chunk);
        }

        (i, &self.active_chunks[i..])
    }

    /// Move all chunks that the GPU has finished using back to the unused
    /// chunks.
    fn receive_free(&mut self) -> (usize, &[Buffer]) {
        let i = self.unused_chunks.len();

        while let Ok(mut chunk) = self.free_receiver.try_recv() {
            chunk.offset = 0;
            self.unused_chunks.push(chunk);
        }

        (i, &self.unused_chunks[i..])
    }
}

struct Buffer {
    pub buffer: wgpu::Buffer,
    pub offset: u64,
}

impl Buffer {
    fn can_allocate(&self, size: wgpu::BufferSize, alignment: wgpu::BufferAddress) -> bool {
        let start = self.offset.next_multiple_of(alignment);
        let end = start + size.get();

        end <= self.buffer.size()
    }

    fn allocate(
        &mut self,
        size: wgpu::BufferSize,
        alignment: wgpu::BufferAddress,
    ) -> wgpu::BufferAddress {
        let start = self.offset.next_multiple_of(alignment);
        let end = start + size.get();

        assert!(end <= self.buffer.size());

        self.offset = end;
        start
    }
}
