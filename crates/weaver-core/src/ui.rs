use std::sync::Arc;

use egui::Context;
use egui_wgpu::renderer::ScreenDescriptor;
use egui_winit::State;
use weaver_util::lock::Lock;
use winit::window::Window;

use super::texture::{TextureFormat, WindowTexture};

pub mod builtin {
    use std::collections::VecDeque;

    use egui_plot::Line;

    #[derive(Clone)]
    pub struct FpsDisplay {
        last_frame: std::time::Instant,
        last_update: std::time::Instant,
        update_interval: std::time::Duration,
        last_print: std::time::Instant,
        print_interval: std::time::Duration,
        history: VecDeque<f32>,
        fps_buffer: Vec<f32>,
        fps: f32,
    }

    impl FpsDisplay {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Self {
            Self {
                last_frame: std::time::Instant::now(),
                last_update: std::time::Instant::now(),
                last_print: std::time::Instant::now(),
                update_interval: std::time::Duration::from_millis(50),
                print_interval: std::time::Duration::from_secs(2),
                history: VecDeque::new(),
                fps_buffer: Vec::new(),
                fps: 0.0,
            }
        }

        pub fn run_ui(&mut self, ctx: &egui::Context) {
            let now = std::time::Instant::now();

            let delta = now - self.last_frame;
            self.last_frame = now;

            let frame_time = delta.as_secs_f32();
            let fps = 1.0 / frame_time;
            self.fps_buffer.push(fps);

            if now - self.last_update > self.update_interval {
                self.last_update = now;
                self.fps = self.fps_buffer.iter().sum::<f32>() / self.fps_buffer.len() as f32;
                self.fps_buffer.clear();
                self.history.push_back(self.fps);
                if self.history.len() > 500 {
                    self.history.pop_front();
                }
            }

            if now - self.last_print > self.print_interval {
                self.last_print = now;
                log::info!("FPS: {:.2}", self.fps);
            }

            let line = Line::new(
                self.history
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(i, fps)| [i as f64, fps as f64])
                    .collect::<Vec<_>>(),
            )
            .color(egui::Color32::from_rgb(0, 255, 0));

            egui::Window::new("FPS")
                .default_height(100.0)
                .default_width(200.0)
                .show(ctx, |ui| {
                    ui.vertical_centered_justified(|ui| {
                        ui.heading(format!("FPS: {:.2}", self.fps));
                    });
                    egui_plot::Plot::new("FPS").show(ui, |plot| plot.line(line))
                });
        }
    }
}

pub struct EguiContext {
    state: Arc<Lock<State>>,
    renderer: Arc<Lock<egui_wgpu::Renderer>>,
    full_output: Arc<Lock<Option<egui::FullOutput>>>,
    locked: bool,
}

impl Clone for EguiContext {
    fn clone(&self) -> Self {
        unimplemented!("EguiContext is not cloneable")
    }
}

impl EguiContext {
    pub fn new(device: &wgpu::Device, window: &Window, msaa_samples: u32) -> Self {
        let ctx = Context::default();
        let viewport_id = ctx.viewport_id();
        let state = State::new(ctx, viewport_id, window, None, None);
        let renderer = egui_wgpu::Renderer::new(device, WindowTexture::FORMAT, None, msaa_samples);
        Self {
            state: Arc::new(Lock::new(state)),
            renderer: Arc::new(Lock::new(renderer)),
            full_output: Arc::new(Lock::new(None)),
            locked: false,
        }
    }

    pub fn lock(&mut self) {
        self.locked = true;
    }

    pub fn unlock(&mut self) {
        self.locked = false;
    }

    pub fn available_rect(&self) -> egui::Rect {
        self.state.read().egui_ctx().available_rect()
    }

    pub fn handle_input(&self, window: &Window, event: &winit::event::WindowEvent) {
        let _ = self.state.write().on_window_event(window, event);
    }

    pub fn wants_focus(&self) -> bool {
        self.state.read().egui_ctx().wants_keyboard_input()
            || self.state.read().egui_ctx().wants_pointer_input()
    }

    pub fn begin_frame(&self, window: &Window) {
        if self.full_output.read().is_none() {
            let raw_input = self.state.write().take_egui_input(window);
            self.state.read().egui_ctx().begin_frame(raw_input);
        }
    }

    pub fn end_frame(&self) {
        if self.full_output.read().is_none() {
            *self.full_output.write() = Some(self.state.read().egui_ctx().end_frame());
        }
    }

    pub fn draw_if_ready<F: FnOnce(&Context) -> R, R>(&self, f: F) {
        if self.full_output.read().is_none() && !self.locked {
            f(self.state.read().egui_ctx());
        }
    }

    pub fn convert_texture(
        &self,
        device: &wgpu::Device,
        texture: &wgpu::TextureView,
    ) -> egui::epaint::TextureId {
        self.renderer
            .write()
            .register_native_texture(device, texture, wgpu::FilterMode::Nearest)
    }

    pub fn update_texture(
        &self,
        device: &wgpu::Device,
        texture: &wgpu::TextureView,
        id: egui::epaint::TextureId,
    ) {
        self.renderer.write().update_egui_texture_from_wgpu_texture(
            device,
            texture,
            wgpu::FilterMode::Nearest,
            id,
        );
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        window: &Window,
        window_surface_view: &wgpu::TextureView,
        screen_descriptor: &ScreenDescriptor,
    ) {
        if self.full_output.read().is_none() {
            return;
        }
        let full_output = self.full_output.write().take().unwrap();
        let pixels_per_point = screen_descriptor.pixels_per_point;

        self.state
            .write()
            .handle_platform_output(window, full_output.platform_output);

        let tris = self
            .state
            .read()
            .egui_ctx()
            .tessellate(full_output.shapes, pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .write()
                .update_texture(device, queue, *id, image_delta);
        }
        self.renderer
            .write()
            .update_buffers(device, queue, encoder, &tris, screen_descriptor);

        let renderer = self.renderer.read();
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("egui render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: window_surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        renderer.render(&mut render_pass, &tris, screen_descriptor);
        drop(render_pass);
        drop(renderer);
        for x in &full_output.textures_delta.free {
            self.renderer.write().free_texture(x);
        }
    }
}