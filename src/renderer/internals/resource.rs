use std::{
    cell::{Ref, RefCell},
    fmt::Debug,
    rc::Rc,
    sync::Arc,
};

use super::{BindGroupLayoutCache, BindableComponent, GpuResourceManager};

/// The type of a GPU resource.
/// This is used to create the appropriate binding type.
/// This is also used to properly initialize a `LazyGpuHandle`.
#[derive(Clone)]
pub enum GpuResourceType {
    Uniform {
        usage: wgpu::BufferUsages,
        size: usize,
    },
    Storage {
        usage: wgpu::BufferUsages,
        size: usize,
        read_only: bool,
    },
    Texture {
        width: u32,
        height: u32,
        usage: wgpu::TextureUsages,
        format: wgpu::TextureFormat,
        dimension: wgpu::TextureDimension,
        view_dimension: wgpu::TextureViewDimension,
        depth_or_array_layers: u32,
    },
    Sampler {
        address_mode: wgpu::AddressMode,
        filter_mode: wgpu::FilterMode,
        compare: Option<wgpu::CompareFunction>,
    },
}

impl Debug for GpuResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Uniform { .. } => write!(f, "Uniform"),
            Self::Storage { .. } => write!(f, "Storage"),
            Self::Texture { .. } => write!(f, "Texture"),
            Self::Sampler { .. } => write!(f, "Sampler"),
        }
    }
}

impl Into<wgpu::BindingType> for &GpuResourceType {
    fn into(self) -> wgpu::BindingType {
        match self {
            GpuResourceType::Uniform { .. } => wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            GpuResourceType::Storage { read_only, .. } => wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage {
                    read_only: *read_only,
                },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            GpuResourceType::Texture { view_dimension, .. } => wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: *view_dimension,
                multisampled: false,
            },
            GpuResourceType::Sampler {
                filter_mode,
                compare,
                ..
            } => {
                let comparison = compare.is_some();
                let filtering = filter_mode != &wgpu::FilterMode::Nearest;
                if comparison {
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison)
                } else if filtering {
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
                } else {
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering)
                }
            }
        }
    }
}

/// A GPU-allocated resource.
pub enum GpuResource {
    Buffer { buffer: wgpu::Buffer },
    Texture { texture: wgpu::Texture },
    Sampler { sampler: wgpu::Sampler },
}

/// A handle to a GPU-allocated resource.
#[derive(Clone)]
pub struct GpuHandle {
    pub id: u64,
    pub status: Rc<RefCell<GpuHandleStatus>>,
}

impl GpuHandle {
    /// Marks the handle as pending an update.
    /// This will not update the GPU resource until the next frame, unless the render queue is manually flushed.
    /// See [`GpuResourceManager`] for more information.
    pub fn update<T: bytemuck::Pod>(&mut self, data: &[T]) {
        let mut status = self.status.borrow_mut();
        if !status.is_destroyed() {
            *status = GpuHandleStatus::Pending {
                pending_data: Arc::from(bytemuck::cast_slice(data)),
            };
        } else {
            log::warn!(
                "Attempted to update buffer that is already destroyed: {} is {:?}",
                self.id,
                self.status
            );
        }
    }

    /// Returns the underlying buffer iff the handle is ready and the underlying resource is a buffer.
    pub fn get_buffer(&self) -> Option<Ref<'_, wgpu::Buffer>> {
        let status = self.status.borrow();
        if let GpuHandleStatus::Ready {
            resource: ref buffer,
        } = &*status
        {
            match buffer.as_ref() {
                GpuResource::Buffer { .. } => Some(Ref::map(status, |status| match status {
                    GpuHandleStatus::Ready { resource: buffer } => match buffer.as_ref() {
                        GpuResource::Buffer { buffer } => buffer,
                        _ => unreachable!(),
                    },
                    _ => unreachable!(),
                })),
                GpuResource::Texture { .. } => {
                    log::warn!(
                        "Attempted to get buffer from texture: {} is {:?}",
                        self.id,
                        self.status
                    );
                    None
                }
                GpuResource::Sampler { .. } => {
                    log::warn!(
                        "Attempted to get buffer from sampler: {} is {:?}",
                        self.id,
                        self.status
                    );
                    None
                }
            }
        } else {
            log::warn!(
                "Attempted to get buffer that is not ready: {} is {:?}",
                self.id,
                self.status
            );
            None
        }
    }

    /// Returns the underlying texture iff the handle is ready and the underlying resource is a texture.
    pub fn get_texture(&self) -> Option<Ref<'_, wgpu::Texture>> {
        let status = self.status.borrow();
        if let GpuHandleStatus::Ready {
            resource: ref buffer,
        } = &*status
        {
            match buffer.as_ref() {
                GpuResource::Texture { .. } => Some(Ref::map(status, |status| match status {
                    GpuHandleStatus::Ready { resource: buffer } => match buffer.as_ref() {
                        GpuResource::Texture { texture, .. } => texture,
                        _ => unreachable!(),
                    },
                    _ => unreachable!(),
                })),
                GpuResource::Buffer { .. } => {
                    log::warn!(
                        "Attempted to get a texture from buffer: {} is {:?}",
                        self.id,
                        self.status
                    );
                    None
                }
                GpuResource::Sampler { .. } => {
                    log::warn!(
                        "Attempted to get a texture from sampler: {} is {:?}",
                        self.id,
                        self.status
                    );
                    None
                }
            }
        } else {
            log::warn!(
                "Attempted to get a texture that is not ready: {} is {:?}",
                self.id,
                self.status
            );
            None
        }
    }

    pub fn get_sampler(&self) -> Option<Ref<'_, wgpu::Sampler>> {
        let status = self.status.borrow();
        if let GpuHandleStatus::Ready {
            resource: ref buffer,
        } = &*status
        {
            match buffer.as_ref() {
                GpuResource::Sampler { .. } => Some(Ref::map(status, |status| match status {
                    GpuHandleStatus::Ready { resource: buffer } => match buffer.as_ref() {
                        GpuResource::Sampler { sampler } => sampler,
                        _ => unreachable!(),
                    },
                    _ => unreachable!(),
                })),
                GpuResource::Buffer { .. } => {
                    log::warn!(
                        "Attempted to get a sampler from buffer: {} is {:?}",
                        self.id,
                        self.status
                    );
                    None
                }
                GpuResource::Texture { .. } => {
                    log::warn!(
                        "Attempted to get a sampler from texture: {} is {:?}",
                        self.id,
                        self.status
                    );
                    None
                }
            }
        } else {
            log::warn!(
                "Attempted to get a sampler that is not ready: {} is {:?}",
                self.id,
                self.status
            );
            None
        }
    }

    /// Marks the underlying GPU resource as pending destruction, if it is not already destroyed.
    /// This will not destroy the GPU resource until the next frame, unless [`GpuResourceManager::gc_destroyed_buffers`] is manually called.
    pub fn destroy(&mut self) {
        let mut status = self.status.borrow_mut();
        match &mut *status {
            GpuHandleStatus::Ready { .. } => {
                *status = GpuHandleStatus::Destroyed;
            }
            GpuHandleStatus::Pending { .. } => {
                *status = GpuHandleStatus::Destroyed;
            }
            GpuHandleStatus::Destroyed => {
                log::warn!("Attempted to destroy a buffer that is already destroyed");
            }
        }
    }
}

/// The status of a lazily initialized resource.

pub enum LazyInitStatus {
    /// The resource is ready to be used.
    Ready { handle: GpuHandle },
    /// The resource is pending initialization.
    Pending {
        ty: GpuResourceType,
        label: Option<&'static str>,
        pending_data: Option<Arc<[u8]>>,
    },
    /// The resource has been destroyed.
    Destroyed,
}

impl Debug for LazyInitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LazyInitStatus::Ready { .. } => write!(f, "Ready"),
            LazyInitStatus::Pending { ty, .. } => write!(f, "Pending ({:#?})", ty),
            LazyInitStatus::Destroyed => write!(f, "Destroyed"),
        }
    }
}

/// A handle to a GPU resource that is lazily initialized.
/// This is useful for resources that are not used by the GPU until the first frame.
#[derive(Clone, Debug)]
pub struct LazyGpuHandle {
    status: Rc<RefCell<LazyInitStatus>>,
}

impl LazyGpuHandle {
    /// Creates a new `LazyGpuHandle` with the given resource type, and optional label and pending data.
    pub(crate) fn new(
        ty: GpuResourceType,
        label: Option<&'static str>,
        pending_data: Option<Arc<[u8]>>,
    ) -> Self {
        Self {
            status: Rc::new(RefCell::new(LazyInitStatus::Pending {
                ty,
                label,
                pending_data,
            })),
        }
    }

    pub(crate) fn new_ready(handle: GpuHandle) -> Self {
        Self {
            status: Rc::new(RefCell::new(LazyInitStatus::Ready { handle })),
        }
    }

    /// Initializes the underlying GPU resource if it is not already initialized and returns a handle to it.
    /// If the resource is already initialized, this will return a handle to the existing resource without allocating anything new on the GPU.
    pub fn lazy_init(&self, manager: &GpuResourceManager) -> anyhow::Result<GpuHandle> {
        let mut status = self.status.borrow_mut();
        match &mut *status {
            LazyInitStatus::Ready { handle } => Ok(handle.clone()),
            LazyInitStatus::Pending {
                ty,
                label,
                pending_data,
            } => match pending_data {
                Some(pending_data) => {
                    let buffer = match ty {
                        GpuResourceType::Uniform { usage, .. } => {
                            manager.create_buffer_init(&*pending_data, *usage, *label)
                        }
                        GpuResourceType::Storage { usage, .. } => {
                            manager.create_buffer_init(&*pending_data, *usage, *label)
                        }
                        GpuResourceType::Texture {
                            width,
                            height,
                            usage,
                            format,
                            dimension,
                            depth_or_array_layers,
                            ..
                        } => manager.create_texture_init::<_>(
                            *width,
                            *height,
                            *format,
                            *dimension,
                            *depth_or_array_layers,
                            *usage,
                            *label,
                            &*pending_data,
                        ),
                        GpuResourceType::Sampler {
                            address_mode,
                            filter_mode,
                            compare,
                        } => {
                            log::warn!("Attempted to initialize a sampler with pending data");
                            manager.create_sampler(*address_mode, *filter_mode, *compare, *label)
                        }
                    };

                    let handle = manager.insert_resource(buffer);

                    *status = LazyInitStatus::Ready {
                        handle: handle.clone(),
                    };
                    Ok(handle)
                }
                None => {
                    let buffer = match ty {
                        GpuResourceType::Uniform { usage, size } => {
                            manager.create_buffer(*size, *usage, *label)
                        }
                        GpuResourceType::Storage { usage, size, .. } => {
                            manager.create_buffer(*size, *usage, *label)
                        }
                        GpuResourceType::Texture {
                            width,
                            height,
                            usage,
                            format,
                            dimension,
                            depth_or_array_layers,
                            ..
                        } => manager.create_texture(
                            *width,
                            *height,
                            *format,
                            *dimension,
                            *depth_or_array_layers,
                            *usage,
                            *label,
                        ),
                        GpuResourceType::Sampler {
                            address_mode,
                            filter_mode,
                            compare,
                        } => manager.create_sampler(*address_mode, *filter_mode, *compare, *label),
                    };

                    let handle = manager.insert_resource(buffer);

                    *status = LazyInitStatus::Ready {
                        handle: handle.clone(),
                    };
                    Ok(handle)
                }
            },
            LazyInitStatus::Destroyed => {
                log::warn!("Attempted to initialize a destroyed GPU resource");
                Err(anyhow::anyhow!("GPU Resource is destroyed"))
            }
        }
    }

    /// Marks the underlying GPU resource as pending an update, if it is not already destroyed.
    /// This will not update the GPU resource until the next frame, unless the render queue is manually flushed.
    pub fn update<T: bytemuck::Pod>(&self, data: &[T]) {
        let mut status = self.status.borrow_mut();
        match &mut *status {
            LazyInitStatus::Ready { handle } => {
                handle.update(data);
            }
            LazyInitStatus::Pending { pending_data, .. } => {
                *pending_data = Some(Arc::from(bytemuck::cast_slice(data)));
            }
            LazyInitStatus::Destroyed => {
                log::warn!("Attempted to update a destroyed buffer");
            }
        }
    }

    /// Marks the underlying GPU resource as pending destruction, if it is not already destroyed.
    /// This will not destroy the GPU resource until the next frame, unless [`GpuResourceManager::gc_destroyed_buffers`] is manually called.
    pub fn destroy(&self) {
        let mut status = self.status.borrow_mut();
        match &mut *status {
            LazyInitStatus::Ready { handle } => {
                handle.destroy();
            }
            LazyInitStatus::Pending { .. } => {
                *status = LazyInitStatus::Destroyed;
            }
            LazyInitStatus::Destroyed => {
                log::warn!("Attempted to destroy an already destroyed buffer");
            }
        }
    }
}

#[derive(Clone)]
pub enum GpuHandleStatus {
    Ready { resource: Arc<GpuResource> },
    Pending { pending_data: Arc<[u8]> },
    Destroyed,
}

impl GpuHandleStatus {
    pub fn is_ready(&self) -> bool {
        matches!(self, GpuHandleStatus::Ready { .. })
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, GpuHandleStatus::Pending { .. })
    }

    pub fn is_destroyed(&self) -> bool {
        matches!(self, GpuHandleStatus::Destroyed)
    }
}

impl Debug for GpuHandleStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuHandleStatus::Ready { .. } => write!(f, "Ready"),
            GpuHandleStatus::Pending { .. } => write!(f, "Pending"),
            GpuHandleStatus::Destroyed => write!(f, "Destroyed"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LazyBindGroup<T: BindableComponent> {
    pub layout: Rc<RefCell<Option<Arc<wgpu::BindGroupLayout>>>>,
    pub bind_group: Rc<RefCell<Option<Arc<wgpu::BindGroup>>>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> Default for LazyBindGroup<T>
where
    T: BindableComponent,
{
    fn default() -> Self {
        Self {
            layout: Rc::new(RefCell::new(None)),
            bind_group: Rc::new(RefCell::new(None)),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T> LazyBindGroup<T>
where
    T: BindableComponent,
{
    pub fn is_initialized(&self) -> bool {
        self.layout.borrow().is_some() && self.bind_group.borrow().is_some()
    }

    pub fn bind_group(&self) -> Option<Arc<wgpu::BindGroup>> {
        self.bind_group.borrow().as_ref().cloned()
    }

    pub fn bind_group_layout(&self) -> Option<Arc<wgpu::BindGroupLayout>> {
        self.layout.borrow().as_ref().cloned()
    }

    pub fn lazy_init_layout(
        &self,
        manager: &GpuResourceManager,
        cache: &BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroupLayout>> {
        let mut layout = self.layout.borrow_mut();
        if layout.is_none() {
            *layout = Some(cache.get_or_create::<T>(manager.device()));
        }
        Ok(layout.as_ref().unwrap().clone())
    }

    pub fn lazy_init_bind_group(
        &self,
        manager: &GpuResourceManager,
        cache: &BindGroupLayoutCache,
        component: &T,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
        let mut layout = self.layout.borrow_mut();
        let mut bind_group = self.bind_group.borrow_mut();
        if layout.is_none() {
            *layout = Some(cache.get_or_create::<T>(manager.device()));
        }
        if bind_group.is_none() {
            *bind_group = Some(component.create_bind_group(manager, cache)?);
        }
        Ok(bind_group.as_ref().unwrap().clone())
    }
}
