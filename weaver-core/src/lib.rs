#![allow(clippy::too_many_arguments, clippy::from_over_into)]

use std::sync::Arc;

use weaver_ecs::{component::Component, registry::Registry};

use crate::camera::FlyCameraController;

pub mod aabb;
pub mod app;
pub mod asset_server;
pub mod camera;
pub mod color;
pub mod doodads;
pub mod input;
pub mod light;
pub mod material;
pub mod mesh;
pub mod model;
pub mod particles;
pub mod physics;
pub mod renderer;
pub mod texture;
pub mod time;
pub mod transform;
pub mod ui;

pub mod prelude {
    pub use crate::{
        aabb::Aabb,
        app::App,
        asset_server::AssetServer,
        camera::Camera,
        color::Color,
        doodads::{Cone, Cube, Doodad, Doodads},
        input::{Input, KeyCode, MouseButton},
        light::{DirectionalLight, PointLight},
        material::Material,
        mesh::Mesh,
        model::{ModelBundle, RigidBodyModelBundle},
        particles::ParticleEmitter,
        physics::{RapierContext, RigidBody},
        renderer::Renderer,
        texture::{Texture, TextureFormat},
        time::Time,
        transform::Transform,
        ui::EguiContext,
    };
    pub use weaver_proc_macro::{Bundle, Component};
}

pub(crate) fn register_all(registry: &Arc<Registry>) {
    use crate::prelude::*;
    registry.get_static::<AssetServer>();
    registry.get_static::<Camera>();
    registry.get_static::<Color>();
    registry.get_static::<Cone>();
    registry.get_static::<Cube>();
    registry.get_static::<Doodads>();
    registry.get_static::<DirectionalLight>();
    registry.get_static::<Input>();
    registry.get_static::<Material>();
    registry.get_static::<Mesh>();
    registry.get_static::<ParticleEmitter>();
    registry.get_static::<PointLight>();
    registry.get_static::<RapierContext>();
    registry.get_static::<RigidBody>();
    registry.get_static::<Renderer>();
    registry.get_static::<Texture>();
    registry.get_static::<Time>();
    registry.get_static::<Transform>();
    registry.get_static::<EguiContext>();

    Transform::register_methods(registry);
    Camera::register_methods(registry);
    FlyCameraController::register_methods(registry);
    Time::register_methods(registry);
    Input::register_methods(registry);
    PointLight::register_methods(registry);
    Color::register_methods(registry);
    AssetServer::register_methods(registry);
}
