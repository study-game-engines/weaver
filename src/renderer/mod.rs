use weaver_ecs::World;
use winit::window::Window;

use crate::core::texture::Texture;

use self::pass::{hdr::HdrRenderPass, pbr::PbrRenderPass, Pass};

pub mod pass;

pub struct Renderer {
    pub(crate) surface: wgpu::Surface,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,

    pub(crate) color_texture: Texture,
    pub(crate) depth_texture: Texture,
    pub(crate) normal_texture: Texture,

    pub(crate) hdr_pass: HdrRenderPass,
    pub(crate) pbr_pass: PbrRenderPass,
    pub(crate) passes: Vec<Box<dyn pass::Pass>>,
}

impl Renderer {
    pub async fn new(window: &Window) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = unsafe { instance.create_surface(window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::all_webgpu_mask(),
                    limits: wgpu::Limits::downlevel_defaults(),
                },
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = Texture::WINDOW_FORMAT;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Immediate,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let color_texture = Texture::create_color_texture(
            &device,
            config.width as usize,
            config.height as usize,
            Some("Color Texture"),
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            Some(Texture::WINDOW_FORMAT),
        );

        let depth_texture = Texture::create_depth_texture(
            &device,
            config.width as usize,
            config.height as usize,
            Some("Depth Texture"),
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
        );

        let normal_texture = Texture::create_normal_texture(
            &device,
            config.width as usize,
            config.height as usize,
            Some("Normal Texture"),
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
        );

        let hdr_pass = HdrRenderPass::new(&device, config.width, config.height);

        let pbr_pass = PbrRenderPass::new(&device);

        Self {
            surface,
            device,
            queue,
            config,
            color_texture,
            depth_texture,
            normal_texture,
            hdr_pass,
            pbr_pass,
            passes: vec![],
        }
    }

    pub fn push_render_pass<T: Pass + 'static>(&mut self, pass: T) {
        self.passes.push(Box::new(pass));
    }

    pub fn render(&mut self, world: &World) -> anyhow::Result<()> {
        let output = self.surface.get_current_texture()?;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        // clear the screen
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Screen"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.hdr_pass.texture.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: true,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.normal_texture.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: true,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        self.pbr_pass.render(
            &self.device,
            &self.queue,
            &self.hdr_pass.texture,
            &self.normal_texture,
            &self.depth_texture,
            world,
        )?;

        for pass in self.passes.iter() {
            pass.render(
                &self.device,
                &self.queue,
                &self.hdr_pass.texture,
                &self.normal_texture,
                &self.depth_texture,
                world,
            )?;
        }

        self.hdr_pass.render(
            &self.device,
            &self.queue,
            &self.color_texture,
            &self.normal_texture,
            &self.depth_texture,
            world,
        )?;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Copy Color Texture Encoder"),
            });

        // copy color texture to the output
        encoder.copy_texture_to_texture(
            self.color_texture.texture.as_image_copy(),
            output.texture.as_image_copy(),
            wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
