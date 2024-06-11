use std::rc::Rc;

use weaver::{
    prelude::*,
    weaver_app::App,
    weaver_core::{
        input::InputPlugin,
        mesh::Mesh,
        time::{Time, TimePlugin},
    },
    weaver_ecs::{system::SystemStage, world::World},
    weaver_pbr::{camera::PbrCamera, material::Material, PbrPlugin},
    weaver_renderer::{camera::Camera, RendererPlugin},
    weaver_winit::WinitPlugin,
};
use weaver_core::CoreTypesPlugin;
use weaver_diagnostics::frame_time::LogFrameTimePlugin;
use weaver_egui::{prelude::egui, EguiContext, EguiPlugin};

pub mod camera;

fn main() -> Result<()> {
    env_logger::init();
    App::new()?
        .add_plugin(CoreTypesPlugin)?
        .add_plugin(WinitPlugin {
            initial_size: (1280, 720),
        })?
        .add_plugin(TimePlugin)?
        .add_plugin(InputPlugin)?
        .add_plugin(AssetPlugin)?
        .add_plugin(RendererPlugin)?
        .add_plugin(PbrPlugin)?
        .add_plugin(EguiPlugin)?
        .add_plugin(LogFrameTimePlugin {
            log_interval: std::time::Duration::from_secs(1),
        })?
        .add_system(setup, SystemStage::Init)?
        .add_system(update, SystemStage::Update)?
        .add_system(camera::update_camera, SystemStage::Update)?
        .add_system(ui, SystemStage::Ui)?
        .run()
}

fn setup(world: Rc<World>) -> Result<()> {
    let scene = world.root_scene();
    let _camera = scene.spawn((
        Camera::perspective_lookat(
            Vec3::new(10.0, 10.0, 10.0),
            Vec3::ZERO,
            Vec3::Y,
            45.0f32.to_radians(),
            1280.0 / 720.0,
            0.1,
            100.0,
        ),
        PbrCamera::new(Color::new(0.1, 0.1, 0.1, 1.0)),
        *camera::FlyCameraController {
            aspect: 1280.0 / 720.0,
            ..Default::default()
        }
        .look_at(Vec3::new(10.0, 10.0, 10.0), Vec3::ZERO, Vec3::Y),
    ));

    let asset_loader = world.get_resource::<AssetLoader>().unwrap();

    let mesh = asset_loader.load::<Mesh>("assets/meshes/cube.obj")?;

    let material = asset_loader.load::<Material>("assets/materials/wood_tiles.glb")?;
    {
        let mut assets = world.get_resource_mut::<Assets>().unwrap();
        assets.get_mut::<Material>(material).unwrap().texture_scale = 1.0;
    }

    for i in -10..10 {
        for j in -10..10 {
            let _cube = scene.spawn((
                mesh,
                material,
                Transform {
                    translation: Vec3::new(i as f32, 0.0, j as f32),
                    rotation: Quat::IDENTITY,
                    scale: Vec3::new(0.3, 0.3, 0.3),
                },
            ));
        }
    }

    const COLORS: &[Color] = &[
        Color::RED,
        Color::GREEN,
        Color::BLUE,
        Color::YELLOW,
        Color::MAGENTA,
        Color::CYAN,
    ];
    // make a circle of lights
    for (i, color) in COLORS.iter().enumerate() {
        let theta = (i as f32 / COLORS.len() as f32) * std::f32::consts::PI * 2.0;
        let _light = scene.spawn(PointLight {
            position: Vec3::new(10.0 * theta.cos(), 5.0, 10.0 * theta.sin()),
            color: *color,
            intensity: 100.0,
            radius: 100.0,
        });
    }

    Ok(())
}

fn update(world: Rc<World>) -> Result<()> {
    let time = world.get_resource::<Time>().unwrap();
    let query = world.query::<&mut Transform>();

    for (_entity, mut transform) in query.iter() {
        let offset = transform.translation.x * transform.translation.z * 0.1;
        transform.translation.y = 1.0 * (time.total_time + offset / 2.0).sin();
        transform.rotation = Quat::from_rotation_y(time.total_time);
    }

    let query = world.query::<&mut PointLight>();
    let light_count = query.entity_iter().count();
    for (i, (_, mut point_light)) in query.iter().enumerate() {
        let theta = time.total_time * 0.5 + (i as f32 - light_count as f32 / 2.0);
        point_light.position.x = 10.0 * theta.cos();
        point_light.position.z = 10.0 * theta.sin();
    }

    Ok(())
}

fn ui(world: Rc<World>) -> Result<()> {
    let egui_context = world.get_resource::<EguiContext>().unwrap();

    egui_context.draw_if_ready(|ctx| {
        egui::Window::new("Hello World").show(ctx, |ui| {
            ui.label("Hello World!");
            ui.label("This is a test of the emergency broadcast system.");
            ui.label("This is only a test.");
        })
    });

    Ok(())
}
