use std::{cell::RefCell, sync::Arc};

use rustc_hash::FxHashMap;
use weaver_proc_macro::Component;

use crate::{
    app::asset_server::AssetId,
    core::{
        camera::{Camera, CameraUniform},
        light::PointLightArray,
        material::Material,
        mesh::{Mesh, Vertex},
        texture::{DepthTexture, HdrCubeTexture, HdrTexture, Skybox, TextureFormat},
        transform::{Transform, TransformArray},
    },
    ecs::{Query, World},
    include_shader,
    renderer::{
        internals::{
            BindGroupLayoutCache, BindableComponent, GpuComponent, GpuHandle, GpuResourceManager,
            GpuResourceType, LazyBindGroup, LazyGpuHandle,
        },
        Renderer,
    },
};

use super::sky::SKYBOX_CUBEMAP_SIZE;

pub struct UniqueMesh {
    pub mesh: Mesh,
    pub material_bind_group: Arc<wgpu::BindGroup>,
    pub transforms: TransformArray,
}

#[derive(Default, Component)]
pub struct UniqueMeshes {
    pub unique_meshes: FxHashMap<(AssetId, AssetId), UniqueMesh>,
}

impl UniqueMeshes {
    pub fn gather(&mut self, world: &World, renderer: &Renderer) {
        let query = Query::<(&Mesh, &mut Material, &Transform)>::new(world);

        // clear the transforms
        for unique_mesh in self.unique_meshes.values_mut() {
            unique_mesh.transforms.clear();
        }

        for (mesh, material, transform) in query.iter() {
            let unique_mesh = self
                .unique_meshes
                .entry((mesh.asset_id(), material.asset_id()))
                .or_insert_with(|| UniqueMesh {
                    mesh: mesh.clone(),
                    material_bind_group: material
                        .lazy_init_bind_group(
                            &renderer.resource_manager,
                            &renderer.bind_group_layout_cache,
                        )
                        .unwrap(),
                    transforms: TransformArray::new(),
                });

            unique_mesh.transforms.push(&transform);
        }
    }
}

impl GpuComponent for UniqueMeshes {
    fn lazy_init(&self, manager: &GpuResourceManager) -> anyhow::Result<Vec<GpuHandle>> {
        let mut handles = Vec::new();
        for unique_mesh in self.unique_meshes.values() {
            handles.extend(unique_mesh.transforms.lazy_init(manager)?);
        }
        Ok(handles)
    }

    fn update_resources(&self, world: &World) -> anyhow::Result<()> {
        for unique_mesh in self.unique_meshes.values() {
            unique_mesh.transforms.update_resources(world)?;
        }
        Ok(())
    }

    fn destroy_resources(&self) -> anyhow::Result<()> {
        for unique_mesh in self.unique_meshes.values() {
            unique_mesh.transforms.destroy_resources()?;
        }
        Ok(())
    }
}

#[derive(Clone, Component)]
pub struct PbrBuffers {
    pub(crate) camera: LazyGpuHandle,
    pub(crate) env_map: LazyGpuHandle,
    pub(crate) bind_group: LazyBindGroup<Self>,
}

impl PbrBuffers {
    pub fn new() -> Self {
        Self {
            camera: LazyGpuHandle::new(
                GpuResourceType::Uniform {
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    size: std::mem::size_of::<CameraUniform>(),
                },
                Some("PBR Camera"),
                None,
            ),
            env_map: LazyGpuHandle::new(
                GpuResourceType::Texture {
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    format: HdrCubeTexture::FORMAT,
                    width: SKYBOX_CUBEMAP_SIZE,
                    height: SKYBOX_CUBEMAP_SIZE,
                    dimension: wgpu::TextureDimension::D2,
                    view_dimension: wgpu::TextureViewDimension::Cube,
                    depth_or_array_layers: 6,
                },
                Some("PBR Environment Map"),
                None,
            ),
            bind_group: LazyBindGroup::default(),
        }
    }
}

impl Default for PbrBuffers {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuComponent for PbrBuffers {
    fn lazy_init(&self, manager: &GpuResourceManager) -> anyhow::Result<Vec<GpuHandle>> {
        Ok(vec![
            self.camera.lazy_init(manager)?,
            self.env_map.lazy_init(manager)?,
        ])
    }

    fn update_resources(&self, _world: &World) -> anyhow::Result<()> {
        Ok(())
    }

    fn destroy_resources(&self) -> anyhow::Result<()> {
        self.camera.destroy();
        self.env_map.destroy();
        Ok(())
    }
}

impl BindableComponent for PbrBuffers {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PBR Bind Group Layout"),
            entries: &[
                // camera
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // env map
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                // env map sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        })
    }

    fn create_bind_group(
        &self,
        manager: &GpuResourceManager,
        cache: &BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
        let layout = cache.get_or_create::<Self>(manager.device());
        let camera = self.camera.lazy_init(manager)?;
        let env_map = self.env_map.lazy_init(manager)?;

        let env_map = env_map.get_texture().unwrap();
        let env_map_view = env_map.create_view(&wgpu::TextureViewDescriptor {
            label: Some("PBR Environment Map View"),
            format: Some(HdrCubeTexture::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        let env_map_sampler = manager.device().create_sampler(&wgpu::SamplerDescriptor {
            label: Some("PBR Environment Map Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = manager
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: camera.get_buffer().unwrap().as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&env_map_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&env_map_sampler),
                    },
                ],
                label: Some("PBR Bind Group"),
            });

        Ok(Arc::new(bind_group))
    }

    fn bind_group(&self) -> Option<Arc<wgpu::BindGroup>> {
        self.bind_group.bind_group().clone()
    }

    fn lazy_init_bind_group(
        &self,
        manager: &GpuResourceManager,
        cache: &BindGroupLayoutCache,
    ) -> anyhow::Result<Arc<wgpu::BindGroup>> {
        if let Some(bind_group) = self.bind_group.bind_group() {
            return Ok(bind_group);
        }

        let bind_group = self.bind_group.lazy_init_bind_group(manager, cache, self)?;
        Ok(bind_group)
    }
}

pub struct PbrRenderPass {
    pipeline: wgpu::RenderPipeline,
    buffers: PbrBuffers,
    unique_meshes: RefCell<UniqueMeshes>,
}

impl PbrRenderPass {
    pub fn new(device: &wgpu::Device, bind_group_layout_cache: &BindGroupLayoutCache) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PBR Shader"),
            source: wgpu::ShaderSource::Wgsl(include_shader!("pbr.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PBR Pipeline Layout"),
            bind_group_layouts: &[
                // mesh transform
                &bind_group_layout_cache.get_or_create::<TransformArray>(device),
                // camera and env map
                &bind_group_layout_cache.get_or_create::<PbrBuffers>(device),
                // material
                &bind_group_layout_cache.get_or_create::<Material>(device),
                // point lights
                &bind_group_layout_cache.get_or_create::<PointLightArray>(device),
            ],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PBR Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[
                    // color target
                    Some(wgpu::ColorTargetState {
                        format: HdrTexture::FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                ],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthTexture::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let unique_meshes = RefCell::new(UniqueMeshes::default());

        Self {
            pipeline,
            buffers: PbrBuffers::new(),
            unique_meshes,
        }
    }

    pub fn prepare(&self, world: &World, renderer: &Renderer) {
        let mut unique_meshes = self.unique_meshes.borrow_mut();
        unique_meshes.gather(world, renderer);
        unique_meshes.lazy_init(&renderer.resource_manager).unwrap();
        unique_meshes.update_resources(world).unwrap();
        self.buffers.lazy_init(&renderer.resource_manager).unwrap();
    }

    pub fn render(
        &self,
        renderer: &Renderer,
        hdr_pass_view: &wgpu::TextureView,
        world: &World,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        let skybox = Query::<&Skybox>::new(world);
        let skybox = skybox.iter().next().unwrap();

        let skybox_handle = &skybox.texture.lazy_init(&renderer.resource_manager)?;
        let skybox_texture = skybox_handle[0].get_texture().unwrap();

        let camera = Query::<&Camera>::new(world);
        let camera = camera.iter().next().unwrap();

        let camera_handle = &camera.lazy_init(&renderer.resource_manager)?[0];

        let my_handles = self.buffers.lazy_init(&renderer.resource_manager)?;
        let my_camera_buffer = my_handles[0].get_buffer().unwrap();
        let my_env_map_texture = my_handles[1].get_texture().unwrap();

        encoder.copy_buffer_to_buffer(
            &camera_handle.get_buffer().unwrap(),
            0,
            &my_camera_buffer,
            0,
            std::mem::size_of::<CameraUniform>() as u64,
        );

        encoder.copy_texture_to_texture(
            skybox_texture.as_image_copy(),
            my_env_map_texture.as_image_copy(),
            skybox_texture.size(),
        );

        let buffer_bind_group = self.buffers.lazy_init_bind_group(
            &renderer.resource_manager,
            &renderer.bind_group_layout_cache,
        )?;

        let point_lights_bind_group = renderer.point_lights.lazy_init_bind_group(
            &renderer.resource_manager,
            &renderer.bind_group_layout_cache,
        )?;

        for unique_mesh in self.unique_meshes.borrow().unique_meshes.values() {
            let UniqueMesh {
                mesh,
                material_bind_group,
                transforms,
            } = unique_mesh;

            let transform_bind_group = transforms.lazy_init_bind_group(
                &renderer.resource_manager,
                &renderer.bind_group_layout_cache,
            )?;

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PBR Render Pass"),
                color_attachments: &[
                    // color target
                    Some(wgpu::RenderPassColorAttachment {
                        view: hdr_pass_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &renderer.depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &transform_bind_group, &[]);
            render_pass.set_bind_group(1, &buffer_bind_group, &[]);
            render_pass.set_bind_group(2, material_bind_group, &[]);
            render_pass.set_bind_group(3, &point_lights_bind_group, &[]);
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
            render_pass.set_index_buffer(mesh.index_buffer().slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..mesh.num_indices() as u32, 0, 0..transforms.len() as u32);
        }

        Ok(())
    }
}
