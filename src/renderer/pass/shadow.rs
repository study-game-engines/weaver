use std::num::NonZeroU32;

use super::Pass;
use crate::{
    core::{
        camera::{CameraUniform, FlyCamera},
        light::{DirectionalLight, DirectionalLightUniform, PointLight, PointLightUniform},
        mesh::{Mesh, Vertex},
        texture::Texture,
        transform::Transform,
    },
    include_shader,
};
use weaver_ecs::{Query, Queryable, Read, World};

const SHADOW_DEPTH_TEXTURE_SIZE: u32 = 2048;

pub struct ShadowRenderPass {
    // the first stage creates the shadow map
    shadow_map_pipeline_layout: wgpu::PipelineLayout,
    shadow_map_pipeline: wgpu::RenderPipeline,
    shadow_map_bind_group_layout: wgpu::BindGroupLayout,
    shadow_map_bind_group: wgpu::BindGroup,

    // the second stage creates the shadow cube map
    shadow_cube_map_pipeline_layout: wgpu::PipelineLayout,
    shadow_cube_map_pipeline: wgpu::RenderPipeline,
    shadow_cube_map_bind_group_layout: wgpu::BindGroupLayout,
    shadow_cube_map_bind_group: wgpu::BindGroup,

    // the third stage overlays the shadow map on the scene
    shadow_overlay_pipeline_layout: wgpu::PipelineLayout,
    shadow_overlay_pipeline: wgpu::RenderPipeline,
    shadow_overlay_bind_group_layout: wgpu::BindGroupLayout,
    shadow_overlay_bind_group: wgpu::BindGroup,

    // the fourth stage overlays the shadow cube map on the scene
    shadow_cube_overlay_pipeline_layout: wgpu::PipelineLayout,
    shadow_cube_overlay_pipeline: wgpu::RenderPipeline,
    shadow_cube_overlay_bind_group_layout: wgpu::BindGroupLayout,
    shadow_cube_overlay_bind_group: wgpu::BindGroup,

    // shadow map texture
    shadow_depth_texture: Texture,
    // shadow cube map texture (for point lights)
    shadow_cube_texture: Texture,
    // shadow cube map individual face views
    shadow_cube_views: Vec<wgpu::TextureView>,
    // shadow cube map depth target cubemap
    shadow_cube_depth_target: Texture,
    // shadow cube map depth target individual face views
    shadow_cube_depth_target_views: Vec<wgpu::TextureView>,
    // copy of the color target, sampled in the third stage
    color_texture: Texture,

    // miscellaneous buffers used in bind groups
    model_transform_buffer: wgpu::Buffer,
    directional_light_buffer: wgpu::Buffer,
    point_light_buffer: wgpu::Buffer,
    point_light_view_transform_buffer: wgpu::Buffer,
    camera_buffer: wgpu::Buffer,
}

impl ShadowRenderPass {
    pub fn new(device: &wgpu::Device, screen_width: u32, screen_height: u32) -> Self {
        let shadow_depth_texture = Texture::create_depth_texture(
            device,
            SHADOW_DEPTH_TEXTURE_SIZE as usize,
            SHADOW_DEPTH_TEXTURE_SIZE as usize,
            Some("Shadow Depth Texture"),
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
        );

        let shadow_cube_texture = Texture::new_hdr_cubemap(
            device,
            SHADOW_DEPTH_TEXTURE_SIZE,
            SHADOW_DEPTH_TEXTURE_SIZE,
            Some(wgpu::TextureViewDimension::Cube),
        );

        let mut shadow_cube_views = Vec::new();
        for i in 0..6 {
            shadow_cube_views.push(shadow_cube_texture.texture.create_view(
                &wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    array_layer_count: None,
                    base_array_layer: i,
                    ..Default::default()
                },
            ));
        }

        let shadow_cube_depth_target =
            Texture::new_depth_cubemap(device, SHADOW_DEPTH_TEXTURE_SIZE);

        let mut shadow_cube_depth_target_views = Vec::new();
        for i in 0..6 {
            shadow_cube_depth_target_views.push(shadow_cube_depth_target.texture.create_view(
                &wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    array_layer_count: None,
                    base_array_layer: i,
                    ..Default::default()
                },
            ));
        }

        let color_texture = Texture::create_color_texture(
            device,
            screen_width as usize,
            screen_height as usize,
            Some("Shadow Color Texture"),
            wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST,
            Some(Texture::HDR_FORMAT),
        );

        let model_transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Model Transform Buffer"),
            size: std::mem::size_of::<glam::Mat4>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let directional_light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Directional Light Buffer"),
            size: std::mem::size_of::<DirectionalLightUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let point_light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Point Light Buffer"),
            size: std::mem::size_of::<PointLightUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let point_light_view_transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Point Light View Transform Buffer"),
            size: std::mem::size_of::<glam::Mat4>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Camera Buffer"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // first stage: create the shadow map

        let shadow_map_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Shadow Map Bind Group Layout"),
                entries: &[
                    // model transform
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // directional light
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let shadow_map_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Map Bind Group"),
            layout: &shadow_map_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: model_transform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: directional_light_buffer.as_entire_binding(),
                },
            ],
        });

        let shadow_map_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shadow Map Pipeline Layout"),
                bind_group_layouts: &[&shadow_map_bind_group_layout],
                push_constant_ranges: &[],
            });

        let shadow_map_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Map Pipeline"),
            layout: Some(&shadow_map_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("Shadow Map Vertex Shader"),
                    source: wgpu::ShaderSource::Wgsl(include_shader!("shadow_map.wgsl").into()),
                }),
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // second stage: create the shadow cube map

        let shadow_cube_map_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Shadow Cube Map Bind Group Layout"),
                entries: &[
                    // model transform
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // point light
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // point light view transform
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let shadow_cube_map_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Cube Map Bind Group"),
            layout: &shadow_cube_map_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: model_transform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: point_light_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: point_light_view_transform_buffer.as_entire_binding(),
                },
            ],
        });

        let shadow_cube_map_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shadow Cube Map Pipeline Layout"),
                bind_group_layouts: &[&shadow_cube_map_bind_group_layout],
                push_constant_ranges: &[],
            });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow Cube Map Shader"),
            source: wgpu::ShaderSource::Wgsl(include_shader!("shadow_cubemap.wgsl").into()),
        });

        let shadow_cube_map_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Shadow Cube Map Pipeline"),
                layout: Some(&shadow_cube_map_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: Texture::HDR_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                // fragment: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Texture::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        // third stage: overlay the shadow map on the scene

        let shadow_overlay_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Shadow Overlay Bind Group Layout"),
                entries: &[
                    // shadow map
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // shadow map sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                        count: None,
                    },
                    // color texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // color texture sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                    // camera uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // directional light uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // model transform
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let shadow_overlay_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Overlay Bind Group"),
            layout: &shadow_overlay_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_depth_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_depth_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&color_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&color_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: directional_light_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: model_transform_buffer.as_entire_binding(),
                },
            ],
        });

        let shadow_overlay_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shadow Overlay Pipeline Layout"),
                bind_group_layouts: &[&shadow_overlay_bind_group_layout],
                push_constant_ranges: &[],
            });

        let shadow_overlay_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Shadow Overlay Pipeline"),
                layout: Some(&shadow_overlay_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("Shadow Overlay Vertex Shader"),
                        source: wgpu::ShaderSource::Wgsl(
                            include_shader!("shadow_overlay.wgsl").into(),
                        ),
                    }),
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("Shadow Overlay Fragment Shader"),
                        source: wgpu::ShaderSource::Wgsl(
                            include_shader!("shadow_overlay.wgsl").into(),
                        ),
                    }),
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: Texture::HDR_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Texture::DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        // fourth stage: overlay the shadow cube map on the scene

        let shadow_cube_overlay_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Shadow Cube Overlay Bind Group Layout"),
                entries: &[
                    // shadow cubemap
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::Cube,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // shadow cubemap sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                    // color texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // color texture sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                    // camera uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // point light uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // model transform
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let shadow_cube_overlay_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Cube Overlay Bind Group"),
            layout: &shadow_cube_overlay_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_cube_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_cube_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&color_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&color_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: point_light_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: model_transform_buffer.as_entire_binding(),
                },
            ],
        });

        let shadow_cube_overlay_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shadow Cube Overlay Pipeline Layout"),
                bind_group_layouts: &[&shadow_cube_overlay_bind_group_layout],
                push_constant_ranges: &[],
            });

        let shadow_cube_overlay_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Shadow Cube Overlay Pipeline"),
                layout: Some(&shadow_cube_overlay_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("Shadow Cube Overlay Vertex Shader"),
                        source: wgpu::ShaderSource::Wgsl(
                            include_shader!("shadow_cubemap_overlay.wgsl").into(),
                        ),
                    }),
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("Shadow Cube Overlay Fragment Shader"),
                        source: wgpu::ShaderSource::Wgsl(
                            include_shader!("shadow_cubemap_overlay.wgsl").into(),
                        ),
                    }),
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: Texture::HDR_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: Texture::DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        Self {
            shadow_map_pipeline_layout,
            shadow_map_pipeline,
            shadow_map_bind_group_layout,
            shadow_map_bind_group,
            shadow_cube_map_pipeline_layout,
            shadow_cube_map_pipeline,
            shadow_cube_map_bind_group_layout,
            shadow_cube_map_bind_group,
            shadow_overlay_pipeline_layout,
            shadow_overlay_pipeline,
            shadow_overlay_bind_group_layout,
            shadow_overlay_bind_group,
            shadow_cube_overlay_pipeline_layout,
            shadow_cube_overlay_pipeline,
            shadow_cube_overlay_bind_group_layout,
            shadow_cube_overlay_bind_group,
            shadow_depth_texture,
            shadow_cube_texture,
            shadow_cube_views,
            shadow_cube_depth_target,
            shadow_cube_depth_target_views,
            color_texture,
            model_transform_buffer,
            directional_light_buffer,
            point_light_buffer,
            point_light_view_transform_buffer,
            camera_buffer,
        }
    }

    fn render_shadow_map(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_target: &Texture,
        depth_target: &Texture,
        world: &World,
    ) -> anyhow::Result<()> {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Shadow Initial Encoder"),
        });

        // clear the shadow map texture
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow Render Pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });
        }

        let camera = world.read_resource::<FlyCamera>();
        let camera_uniform = CameraUniform::from(*camera);

        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );

        let light_query = world.query::<Query<Read<DirectionalLight>>>();
        let directional_light = light_query.iter().next();
        if directional_light.is_none() {
            return Ok(());
        }
        let directional_light = directional_light.unwrap();
        let directional_light_uniform = DirectionalLightUniform::from(&*directional_light);

        queue.write_buffer(
            &self.directional_light_buffer,
            0,
            bytemuck::cast_slice(&[directional_light_uniform]),
        );

        queue.submit(std::iter::once(encoder.finish()));

        let query = world.query::<Query<(Read<Mesh>, Read<Transform>)>>();
        for (mesh, transform) in query.iter() {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shadow Buffer Write Encoder"),
            });

            queue.write_buffer(
                &self.model_transform_buffer,
                0,
                bytemuck::cast_slice(&[transform.matrix]),
            );

            queue.submit(std::iter::once(encoder.finish()));

            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shadow Render Pass Encoder"),
            });

            // build the shadow map
            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Shadow Render Pass"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.shadow_depth_texture.view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        }),
                        stencil_ops: None,
                    }),
                });

                render_pass.set_pipeline(&self.shadow_map_pipeline);
                render_pass.set_bind_group(0, &self.shadow_map_bind_group, &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
            }

            queue.submit(std::iter::once(encoder.finish()));
        }

        Ok(())
    }

    fn render_cube_map(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_target: &Texture,
        depth_target: &Texture,
        world: &World,
    ) -> anyhow::Result<()> {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Shadow Cube Map Initial Encoder"),
        });

        // clear the shadow cubemap texture
        for i in 0..6 {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow Cube Map Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.shadow_cube_views[i],
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0, // f64::MAX?
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_cube_depth_target_views[i],
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });
        }

        let light_query = world.query::<Query<Read<PointLight>>>();
        let point_light = light_query.iter().next().unwrap();
        let point_light_uniform = PointLightUniform::from(&*point_light);

        queue.write_buffer(
            &self.point_light_buffer,
            0,
            bytemuck::cast_slice(&[point_light_uniform]),
        );

        queue.submit(std::iter::once(encoder.finish()));

        let query = world.query::<Query<(Read<Mesh>, Read<Transform>)>>();
        for (mesh, transform) in query.iter() {
            let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shadow Cube Map Buffer Write Encoder"),
            });

            queue.write_buffer(
                &self.model_transform_buffer,
                0,
                bytemuck::cast_slice(&[transform.matrix]),
            );

            queue.submit(std::iter::once(encoder.finish()));
            for i in 0..6 {
                let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Shadow Cube Map Render Pass Encoder"),
                });

                let view_transform = match i {
                    // right
                    0 => point_light.view_transform_in_direction(glam::Vec3::X, glam::Vec3::Y),
                    // left
                    1 => point_light.view_transform_in_direction(-glam::Vec3::X, glam::Vec3::Y),
                    // top
                    2 => point_light.view_transform_in_direction(glam::Vec3::Y, -glam::Vec3::Z),
                    // bottom
                    3 => point_light.view_transform_in_direction(-glam::Vec3::Y, glam::Vec3::Z),
                    // front
                    4 => point_light.view_transform_in_direction(glam::Vec3::Z, glam::Vec3::Y),
                    // back
                    5 => point_light.view_transform_in_direction(-glam::Vec3::Z, glam::Vec3::Y),
                    _ => unreachable!(),
                };

                queue.write_buffer(
                    &self.point_light_view_transform_buffer,
                    0,
                    bytemuck::cast_slice(&[view_transform]),
                );

                queue.submit(std::iter::once(encoder.finish()));

                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Shadow Cube Map Render Pass Encoder"),
                });

                // build the shadow cube map
                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Shadow Cube Map Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &self.shadow_cube_views[i],
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &self.shadow_cube_depth_target_views[i],
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: true,
                            }),
                            stencil_ops: None,
                        }),
                    });

                    render_pass.set_pipeline(&self.shadow_cube_map_pipeline);
                    render_pass.set_bind_group(0, &self.shadow_cube_map_bind_group, &[]);
                    render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    render_pass
                        .set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
                }

                queue.submit(std::iter::once(encoder.finish()));
            }
        }

        Ok(())
    }

    fn overlay_shadow_map(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_target: &Texture,
        depth_target: &Texture,
        world: &World,
    ) -> anyhow::Result<()> {
        let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Shadow Overlay Initial Encoder"),
        });

        let camera = world.read_resource::<FlyCamera>();
        let camera_uniform = CameraUniform::from(*camera);

        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );

        queue.submit(std::iter::once(encoder.finish()));

        let query = world.query::<Query<(Read<Mesh>, Read<Transform>)>>();

        // overlay the built shadow map on the screen
        for (mesh, transform) in query.iter() {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shadow Overlay Buffer Write Encoder"),
            });

            queue.write_buffer(
                &self.model_transform_buffer,
                0,
                bytemuck::cast_slice(&[transform.matrix]),
            );

            // copy the color target to our own copy
            encoder.copy_texture_to_texture(
                color_target.texture.as_image_copy(),
                self.color_texture.texture.as_image_copy(),
                wgpu::Extent3d {
                    width: color_target.texture.width(),
                    height: color_target.texture.height(),
                    depth_or_array_layers: 1,
                },
            );

            queue.submit(std::iter::once(encoder.finish()));

            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shadow Overlay Render Pass Encoder"),
            });

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Shadow Overlay Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &color_target.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &depth_target.view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        }),
                        stencil_ops: None,
                    }),
                });

                render_pass.set_pipeline(&self.shadow_overlay_pipeline);
                render_pass.set_bind_group(0, &self.shadow_overlay_bind_group, &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
            }
            queue.submit(std::iter::once(encoder.finish()));
        }

        Ok(())
    }

    fn overlay_cube_shadow_map(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_target: &Texture,
        depth_target: &Texture,
        world: &World,
    ) -> anyhow::Result<()> {
        let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Shadow Cube Overlay Initial Encoder"),
        });

        let camera = world.read_resource::<FlyCamera>();
        let camera_uniform = CameraUniform::from(*camera);

        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );

        queue.submit(std::iter::once(encoder.finish()));

        let query = world.query::<Query<(Read<Mesh>, Read<Transform>)>>();

        // overlay the built shadow cube map on the screen
        for (mesh, transform) in query.iter() {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shadow Cube Overlay Buffer Write Encoder"),
            });

            queue.write_buffer(
                &self.model_transform_buffer,
                0,
                bytemuck::cast_slice(&[transform.matrix]),
            );

            // copy the color target to our own copy
            encoder.copy_texture_to_texture(
                color_target.texture.as_image_copy(),
                self.color_texture.texture.as_image_copy(),
                wgpu::Extent3d {
                    width: color_target.texture.width(),
                    height: color_target.texture.height(),
                    depth_or_array_layers: 1,
                },
            );

            queue.submit(std::iter::once(encoder.finish()));

            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shadow Cube Overlay Render Pass Encoder"),
            });

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Shadow Cube Overlay Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &color_target.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &depth_target.view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        }),
                        stencil_ops: None,
                    }),
                });

                render_pass.set_pipeline(&self.shadow_cube_overlay_pipeline);
                render_pass.set_bind_group(0, &self.shadow_cube_overlay_bind_group, &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
            }

            queue.submit(std::iter::once(encoder.finish()));
        }

        Ok(())
    }
}

impl Pass for ShadowRenderPass {
    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_target: &Texture,
        depth_target: &Texture,
        world: &World,
    ) -> anyhow::Result<()> {
        self.render_cube_map(device, queue, color_target, depth_target, world)?;
        self.render_shadow_map(device, queue, color_target, depth_target, world)?;
        self.overlay_shadow_map(device, queue, color_target, depth_target, world)?;
        self.overlay_cube_shadow_map(device, queue, color_target, depth_target, world)?;
        Ok(())
    }
}
