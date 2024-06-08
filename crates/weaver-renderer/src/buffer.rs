use std::sync::Arc;

#[derive(Clone)]
pub struct GpuBuffer {
    buffer: Arc<wgpu::Buffer>,
}

impl GpuBuffer {
    pub fn new(buffer: wgpu::Buffer) -> Self {
        Self {
            buffer: Arc::new(buffer),
        }
    }
}

impl std::ops::Deref for GpuBuffer {
    type Target = wgpu::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl From<wgpu::Buffer> for GpuBuffer {
    fn from(buffer: wgpu::Buffer) -> Self {
        Self::new(buffer)
    }
}
