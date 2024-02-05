use crate::{
    asset_server::AssetServer,
    doodads::Doodads,
    input::Input,
    time::{RenderTime, UpdateTime},
    ui::EguiContext,
};

use std::{any::Any, sync::Arc};

use fabricate::prelude::*;

use winit::{event_loop::EventLoop, window::WindowBuilder};

use crate::renderer::{compute::hdr_loader::HdrLoader, Renderer};

#[derive(Clone)]
pub struct Window {
    pub(crate) window: Arc<winit::window::Window>,
    pub fps_mode: bool,
}

impl Window {
    pub fn set_fps_mode(&mut self, fps_mode: bool) {
        self.fps_mode = fps_mode;
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }
}

pub struct App {
    event_loop: EventLoop<()>,
    pub world: LockedWorldHandle,
}

impl App {
    pub fn new(
        title: impl Into<String>,
        screen_width: usize,
        screen_height: usize,
        vsync: bool,
    ) -> anyhow::Result<Self> {
        let world = World::new_handle();

        let event_loop = EventLoop::new()?;
        let window = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(winit::dpi::LogicalSize::new(
                screen_width as f64,
                screen_height as f64,
            ))
            .with_resizable(true)
            .build(&event_loop)?;

        let renderer = Renderer::new(vsync, &window, world.clone());

        let ui = EguiContext::new(renderer.device(), &window, 1);

        let hdr_loader = HdrLoader::new(renderer.device());

        world.write().add_resource(renderer)?;
        world.write().add_resource(hdr_loader)?;
        world.write().add_resource(UpdateTime::new())?;
        world.write().add_resource(RenderTime::new())?;
        world.write().add_resource(Input::default())?;
        world.write().add_resource(ui)?;
        world.write().add_resource(Doodads::default())?;

        let asset_server = AssetServer::new(&world.read())?;
        world.write().add_resource(asset_server)?;

        world.write().add_resource(Window {
            window: Arc::new(window),
            fps_mode: false,
        })?;

        Ok(Self { event_loop, world })
    }

    pub fn add_resource<T: Any + Send + Sync>(&self, resource: T) -> anyhow::Result<()> {
        self.world.write().add_resource(resource)
    }

    pub fn add_system_to_stage<T: System + 'static + Send + Sync>(
        &self,
        system: T,
        stage: SystemStage,
    ) {
        self.world.write().add_system(stage, move |world| {
            system.run(world, &[]).unwrap();
        });
    }

    pub fn add_script(&self, script_path: impl AsRef<std::path::Path>) {
        self.world
            .add_script(Script::load(script_path.as_ref()).unwrap());
    }

    pub fn run(self) -> anyhow::Result<()> {
        self.world.run_systems(SystemStage::Startup);

        // ECS update task
        let (killswitch, killswitch_rx) = crossbeam_channel::bounded(1);
        let (window_event_tx, window_event_rx) = crossbeam_channel::unbounded();
        let (device_event_tx, device_event_rx) = crossbeam_channel::unbounded();
        let update_world = self.world.clone();
        std::thread::Builder::new()
            .name("Weaver ECS Update Loop".to_owned())
            .spawn(move || {
                loop {
                    {
                        let world = update_world.read();
                        let mut input = world.write_resource::<Input>().unwrap();
                        input.prepare_for_update();

                        while let Ok(event) = window_event_rx.try_recv() {
                            input.update_window(&event);

                            let window = world.read_resource::<Window>().unwrap();
                            let ui = world.read_resource::<EguiContext>().unwrap();
                            ui.handle_input(&window.window, &event);
                        }
                        while let Ok(event) = device_event_rx.try_recv() {
                            input.update_device(&event);
                        }
                    }

                    {
                        let world = update_world.read();
                        let mut time = world.write_resource::<UpdateTime>().unwrap();
                        time.update();
                    }

                    update_world.run_systems(SystemStage::PreUpdate);

                    update_world.run_systems(SystemStage::Update);

                    update_world.run_systems(SystemStage::PostUpdate);

                    if killswitch_rx.try_recv().is_ok() {
                        break;
                    }

                    // std::thread::sleep(std::time::Duration::from_millis(1));
                }

                update_world.run_systems(SystemStage::Shutdown);
            })?;

        self.event_loop.run(move |event, target| {
            target.set_control_flow(winit::event_loop::ControlFlow::Poll);

            match event {
                winit::event::Event::LoopExiting => {
                    killswitch.send(()).unwrap();
                }
                winit::event::Event::DeviceEvent { event, .. } => {
                    device_event_tx.send(event.clone()).unwrap();
                }
                winit::event::Event::WindowEvent { event, .. } => {
                    window_event_tx.send(event.clone()).unwrap();
                    match event {
                        winit::event::WindowEvent::CloseRequested => {
                            target.exit();
                        }
                        winit::event::WindowEvent::Resized(size) => {
                            let world = self.world.read();
                            let renderer = world.read_resource::<Renderer>().unwrap();
                            renderer.resize_surface(size.width, size.height);
                        }
                        winit::event::WindowEvent::CursorMoved { .. } => {
                            // center the cursor
                            let world = self.world.read();
                            let window = world.read_resource::<Window>().unwrap();
                            if window.fps_mode {
                                window
                                    .window
                                    .set_cursor_position(winit::dpi::PhysicalPosition::new(
                                        window.window.inner_size().width / 2,
                                        window.window.inner_size().height / 2,
                                    ))
                                    .unwrap();
                                window
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::Confined)
                                    .unwrap();
                                window.window.set_cursor_visible(false);
                            } else {
                                window
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::None)
                                    .unwrap();
                                window.window.set_cursor_visible(true);
                            }
                        }
                        winit::event::WindowEvent::RedrawRequested => {
                            {
                                let world = self.world.read();
                                let mut time = world.write_resource::<RenderTime>().unwrap();
                                time.update();
                            }

                            {
                                let world = self.world.read();
                                let window = world.read_resource::<Window>().unwrap();
                                let gui = world.read_resource::<EguiContext>().unwrap();
                                gui.begin_frame(&window.window);
                            }

                            self.world.run_systems(SystemStage::Ui);

                            {
                                let world = self.world.read();
                                let gui = world.read_resource::<EguiContext>().unwrap();
                                gui.end_frame();
                            }

                            self.world.run_systems(SystemStage::PreRender);

                            self.world.run_systems(SystemStage::Render);

                            self.world.run_systems(SystemStage::PostRender);

                            let world = self.world.read();
                            let window = world.try_read_resource::<Window>();
                            let renderer = world.try_read_resource::<Renderer>();
                            if let (Some(window), Some(renderer)) = (window, renderer) {
                                window.window.pre_present_notify();
                                renderer.present();
                                window.request_redraw();
                            };
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        })?;

        Ok(())
    }
}
