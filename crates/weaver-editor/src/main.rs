use weaver::{
    app::App,
    core::{
        input::InputPlugin,
        mesh::Mesh,
        time::{Time, TimePlugin},
    },
    ecs::{system::SystemStage, world::World},
    pbr::{camera::PbrCamera, material::Material, PbrPlugin},
    prelude::*,
    renderer::{camera::Camera, RendererPlugin},
    winit::WinitPlugin,
};
use weaver_diagnostics::frame_time::LogFrameTimePlugin;

fn main() -> Result<()> {
    env_logger::init();
    App::new()?
        .add_plugin(WinitPlugin {
            initial_size: (1280, 720),
        })?
        .add_plugin(TimePlugin)?
        .add_plugin(InputPlugin)?
        .add_plugin(AssetPlugin)?
        .add_plugin(RendererPlugin)?
        .add_plugin(PbrPlugin)?
        .add_plugin(LogFrameTimePlugin {
            log_interval: std::time::Duration::from_secs(1),
        })?
        .add_system(setup, SystemStage::Init)?
        .add_system(update, SystemStage::Update)?
        .run()
}

fn setup(world: &World) -> Result<()> {
    let scene = world.root_scene();
    let camera = scene.create_node_with(Camera::perspective_lookat(
        Vec3::new(10.0, 10.0, 10.0),
        Vec3::ZERO,
        Vec3::Y,
        45.0f32.to_radians(),
        1280.0 / 720.0,
        0.1,
        100.0,
    ));
    world.insert_component(
        camera.entity(),
        PbrCamera::new(Color::new(0.1, 0.1, 0.1, 1.0)),
    );

    let asset_loader = world.get_resource::<AssetLoader>().unwrap();

    let mesh = asset_loader.load::<Mesh>("assets/meshes/cube.obj")?;

    let material = asset_loader.load::<Material>("assets/materials/metal.glb")?;
    {
        let mut assets = world.get_resource_mut::<Assets>().unwrap();
        assets.get_mut::<Material>(material).unwrap().texture_scale = 1.0;
    }

    for i in -5..5 {
        for j in -5..5 {
            let cube = scene.create_node_with(mesh);
            world.insert_component(cube.entity(), material);

            let transform = Transform {
                translation: Vec3::new(i as f32, 0.0, j as f32),
                rotation: Quat::IDENTITY,
                scale: Vec3::new(0.3, 0.3, 0.3),
            };
            world.insert_component(cube.entity(), transform);
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
        let _light = scene.create_node_with(PointLight {
            position: Vec3::new(10.0 * theta.cos(), 5.0, 10.0 * theta.sin()),
            color: *color,
            intensity: 100.0,
            radius: 100.0,
        });
    }

    Ok(())
}

fn update(world: &World) -> Result<()> {
    let time = world.get_resource::<Time>().unwrap();
    let query = world.query(&Query::new().read::<Transform>());

    for entity in query.iter() {
        let mut transform = world.get_component_mut::<Transform>(entity).unwrap();
        let offset = transform.translation.x - transform.translation.z;
        transform.translation.y = 1.0 * (time.total_time + offset / 2.0).sin();
        transform.rotation = Quat::from_rotation_y(time.total_time);
    }

    let query = world.query(&Query::new().read::<PointLight>());
    let light_count = query.iter().count();

    for (i, entity) in query.iter().enumerate() {
        let mut point_light = world.get_component_mut::<PointLight>(entity).unwrap();
        let theta = time.total_time * 0.5 + (i as f32 - light_count as f32 / 2.0);
        point_light.position.x = 10.0 * theta.cos();
        point_light.position.z = 10.0 * theta.sin();
    }

    Ok(())
}
