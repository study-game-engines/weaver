use weaver_app::{plugin::Plugin, prelude::App, Runner};
use weaver_ecs::system::SystemStage;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

pub mod prelude {
    pub use super::WinitPlugin;
}

pub struct WinitPlugin;

impl Plugin for WinitPlugin {
    fn build(&self, app: &mut App) -> anyhow::Result<()> {
        let event_loop = EventLoop::new()?;
        app.world().insert_resource(event_loop);
        app.set_runner(WinitRunner);
        Ok(())
    }
}

struct WinitRunner;

impl Runner for WinitRunner {
    fn run(&self, app: App) -> anyhow::Result<()> {
        app.run_systems(SystemStage::PreInit)?;
        app.run_systems(SystemStage::Init)?;
        app.run_systems(SystemStage::PostInit)?;

        let event_loop = app.world().remove_resource::<EventLoop<()>>().unwrap();
        let mut handler = WinitApplicationHandler { app };
        event_loop.run_app(&mut handler)?;

        let WinitApplicationHandler { app } = handler;

        app.run_systems(SystemStage::PreShutdown)?;
        app.run_systems(SystemStage::Shutdown)?;
        app.run_systems(SystemStage::PostShutdown)?;

        Ok(())
    }
}

struct WinitApplicationHandler {
    app: App,
}

impl ApplicationHandler for WinitApplicationHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.app.world().has_resource::<Window>() {
            return;
        }

        let window = event_loop
            .create_window(Window::default_attributes())
            .unwrap();

        self.app.world().insert_resource(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(window) = self.app.world().get_resource::<Window>() {
            if window.id() == window_id {
                match event {
                    WindowEvent::CloseRequested => {
                        event_loop.exit();
                    }
                    WindowEvent::RedrawRequested => {
                        self.app.run_systems(SystemStage::PreUpdate).unwrap();
                        self.app.run_systems(SystemStage::Update).unwrap();
                        self.app.run_systems(SystemStage::PostUpdate).unwrap();

                        self.app.run_systems(SystemStage::Ui).unwrap();

                        self.app.run_systems(SystemStage::PreRender).unwrap();
                        self.app.run_systems(SystemStage::Render).unwrap();
                        self.app.run_systems(SystemStage::PostRender).unwrap();

                        window.request_redraw();
                    }
                    _ => {}
                }
            }
        }
    }
}
