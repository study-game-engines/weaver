use weaver::{
    app::App,
    core::mesh::Mesh,
    ecs::{system::SystemStage, world::World},
    pbr::{camera::PbrCamera, material::Material, PbrPlugin},
    prelude::*,
    renderer::{camera::Camera, RendererPlugin},
    winit::WinitPlugin,
};

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let mut app = App::new()?;
    app.add_plugin(WinitPlugin {
        initial_size: (1600, 900),
    })?;
    app.add_plugin(AssetPlugin)?;
    app.add_plugin(RendererPlugin)?;
    app.add_plugin(PbrPlugin)?;

    app.add_system(setup, SystemStage::Init)?;

    app.run()
}

fn setup(world: &World) -> anyhow::Result<()> {
    let scene = world.root_scene();
    let camera = scene.create_node_with(Camera::perspective_lookat(
        Vec3::new(5.0, 5.0, 5.0),
        Vec3::ZERO,
        Vec3::Y,
        45.0,
        1600.0 / 900.0,
        0.1,
        100.0,
    ));
    world.insert_component(camera.entity(), PbrCamera::new(Color::RED));

    let asset_loader = world.get_resource::<AssetLoader>().unwrap();

    let mesh = asset_loader.load::<Mesh>("assets/meshes/cube.obj")?;
    let cube = scene.create_node_with(mesh);

    let material = asset_loader.load::<Material>("assets/materials/wood.glb")?;
    world.insert_component(cube.entity(), material);

    Ok(())
}
