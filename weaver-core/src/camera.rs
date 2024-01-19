use crate::renderer::internals::{GpuResourceType, LazyBindGroup, LazyGpuHandle};

use std::sync::Arc;

use weaver_ecs::World;
use weaver_proc_macro::{BindableComponent, Component, GpuComponent};
use winit::event::MouseButton;
pub use winit::keyboard::KeyCode;

use super::input::Input;

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct CameraUniform {
    pub view: glam::Mat4,
    pub proj: glam::Mat4,
    pub inv_view: glam::Mat4,
    pub inv_proj: glam::Mat4,
    pub camera_position: glam::Vec3,
    pub _padding: u32,
}

impl From<&Camera> for CameraUniform {
    fn from(camera: &Camera) -> Self {
        let view = camera.view_matrix;
        let proj = camera.projection_matrix;
        let inv_view = view.inverse();
        let inv_proj = proj.inverse();
        let camera_position = inv_view.col(3).truncate();

        Self {
            view,
            proj,
            inv_view,
            inv_proj,
            camera_position,
            _padding: 0,
        }
    }
}

#[derive(Component, GpuComponent, BindableComponent)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[gpu(update = "update")]
pub struct Camera {
    pub view_matrix: glam::Mat4,
    pub projection_matrix: glam::Mat4,

    #[cfg_attr(feature = "serde", serde(skip, default = "Camera::default_handle"))]
    #[uniform]
    pub(crate) handle: LazyGpuHandle,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub(crate) bind_group: LazyBindGroup<Self>,
}

impl Camera {
    pub fn new(view_matrix: glam::Mat4, projection_matrix: glam::Mat4) -> Self {
        Self {
            view_matrix,
            projection_matrix,
            handle: Self::default_handle(),
            bind_group: LazyBindGroup::default(),
        }
    }

    fn default_handle() -> LazyGpuHandle {
        LazyGpuHandle::new(
            GpuResourceType::Uniform {
                usage: wgpu::BufferUsages::UNIFORM
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                size: std::mem::size_of::<CameraUniform>(),
            },
            Some("Camera"),
            None,
        )
    }

    pub fn update(&self, _world: &World) -> anyhow::Result<()> {
        self.handle.update(&[CameraUniform::from(self)]);
        Ok(())
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::new(glam::Mat4::IDENTITY, glam::Mat4::IDENTITY)
    }
}

#[derive(Debug, Component, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FlyCameraController {
    pub speed: f32,
    pub sensitivity: f32,
    pub translation: glam::Vec3,
    pub rotation: glam::Quat,
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl FlyCameraController {
    pub fn update(&mut self, input: &Input, delta_time: f32, camera: &mut Camera) {
        let mouse_delta = input.mouse_delta();
        let (mut yaw, mut pitch, _roll) = self.rotation.to_euler(glam::EulerRot::YXZ);

        let forward = self.rotation * glam::Vec3::NEG_Z;
        let right = self.rotation * glam::Vec3::X;

        let mut velocity = glam::Vec3::ZERO;

        if input.key_pressed(KeyCode::KeyW) {
            velocity += forward;
        }
        if input.key_pressed(KeyCode::KeyS) {
            velocity -= forward;
        }
        if input.key_pressed(KeyCode::KeyD) {
            velocity += right;
        }
        if input.key_pressed(KeyCode::KeyA) {
            velocity -= right;
        }
        if input.key_pressed(KeyCode::Space) {
            velocity += glam::Vec3::Y;
        }
        if input.key_pressed(KeyCode::ControlLeft) {
            velocity -= glam::Vec3::Y;
        }

        velocity = velocity.normalize_or_zero() * self.speed * delta_time;

        if input.key_pressed(KeyCode::ShiftLeft) {
            velocity *= 2.0;
        }

        self.translation += velocity;

        if input.mouse_button_pressed(MouseButton::Right) {
            yaw += -(mouse_delta.x * self.sensitivity).to_radians();
            pitch += -(mouse_delta.y * self.sensitivity).to_radians();
        }

        pitch = pitch.clamp(
            -std::f32::consts::FRAC_PI_2 + 0.001,
            std::f32::consts::FRAC_PI_2 - 0.001,
        );

        self.rotation = glam::Quat::from_axis_angle(glam::Vec3::Y, yaw)
            * glam::Quat::from_axis_angle(glam::Vec3::X, pitch);
        self.rotation = self.rotation.normalize();

        camera.view_matrix = self.view_matrix();
        camera.projection_matrix = self.projection_matrix();
    }

    pub fn view_matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_rotation_translation(self.rotation, self.translation).inverse()
    }

    pub fn projection_matrix(&self) -> glam::Mat4 {
        glam::Mat4::perspective_rh(self.fov, self.aspect, self.near, self.far)
    }
}