use ecs::{
    component::{Component, Field},
    system::{Query, ResolvedQuery, System, SystemLogic},
};
use renderer::{
    color::Color,
    mesh::{Mesh, Vertex},
};

pub mod app;
#[macro_use]
pub mod ecs;
pub mod renderer;

fn test_system(queries: &mut [ResolvedQuery]) {
    // rotate any meshes
    let mut dt = 0.0;

    if let ResolvedQuery::Immutable(timer) = &queries[0] {
        for timer in timer {
            dt = match timer.fields.get("dt") {
                Some(Field::F32(dt)) => *dt,
                _ => {
                    log::error!("timer component does not have a f32 dt field");
                    continue;
                }
            };
            log::info!("dt: {}", dt);
        }
    }
    if let ResolvedQuery::Mutable(transforms) = &mut queries[1] {
        for transform in transforms {
            let rotation = match transform.fields.get_mut("rotation") {
                Some(Field::Vec3(rotation)) => rotation,
                _ => {
                    log::error!("transform component does not have a rotation field");
                    continue;
                }
            };
            rotation.x += -2. * dt;
            rotation.y += 1. * dt;
            rotation.z += 3. * dt;
        }
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let mut app = app::App::new((800, 600), "Weaver");

    let e1 = app.world.create_entity();
    let mut mesh1 = Component::new("mesh".to_string());

    // Vertices of a cube.
    #[rustfmt::skip]
    let vertices = vec![
        Vertex {
            position: glam::Vec3::new(-1.0, -1.0, -1.0),
            color: Color::new(1.0, 0.0, 0.0),
        },
        Vertex {
            position: glam::Vec3::new(1.0, -1.0, -1.0),
            color: Color::new(0.0, 1.0, 0.0),
        },
        Vertex {
            position: glam::Vec3::new(-1.0, 1.0, -1.0),
            color: Color::new(0.0, 0.0, 1.0),
        },
        Vertex {
            position: glam::Vec3::new(1.0, 1.0, -1.0),
            color: Color::new(1.0, 1.0, 0.0),
        },
        Vertex {
            position: glam::Vec3::new(-1.0, -1.0, 1.0),
            color: Color::new(1.0, 0.0, 1.0),
        },
        Vertex {
            position: glam::Vec3::new(1.0, -1.0, 1.0),
            color: Color::new(0.0, 1.0, 1.0),
        },
        Vertex {
            position: glam::Vec3::new(-1.0, 1.0, 1.0),
            color: Color::new(1.0, 1.0, 1.0),
        },
        Vertex {
            position: glam::Vec3::new(1.0, 1.0, 1.0),
            color: Color::new(0.0, 0.0, 0.0),
        },
    ];

    // Indices of a cube with counter-clockwise normals.
    #[rustfmt::skip]
    let indices = vec![
        0, 1, 2, 2, 1, 3,
        1, 5, 3, 3, 5, 7,
        5, 4, 7, 7, 4, 6,
        4, 0, 6, 6, 0, 2,
        2, 3, 6, 6, 3, 7,
        4, 5, 0, 0, 5, 1,
    ];

    mesh1.add_field("mesh", Field::Mesh(Mesh::new(vertices, indices)));
    app.world.add_component(e1, mesh1);
    let mut transform1 = Component::new("transform".to_string());
    transform1.add_field("position", Field::Vec3(glam::Vec3::new(0.0, 0.0, 1.0)));
    transform1.add_field("rotation", Field::Vec3(glam::Vec3::new(0.0, 0.0, 0.0)));
    transform1.add_field("scale", Field::Vec3(glam::Vec3::new(0.1, 0.1, 0.1)));
    app.world.add_component(e1, transform1);

    let mut s1 = System::new("test_system".to_string(), SystemLogic::Static(test_system));
    s1.add_query(Query::Immutable("timer".to_string()));
    s1.add_query(Query::Mutable("transform".to_string()));
    app.world.add_system(s1);

    app.run()
}
