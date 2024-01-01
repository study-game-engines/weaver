use weaver_proc_macro::Component;

use super::color::Color;

pub const MAX_LIGHTS: usize = 16;

#[derive(Debug, Clone, Copy, Component)]
pub struct PointLight {
    pub position: glam::Vec3,
    pub color: Color,
    pub intensity: f32,
}

impl PointLight {
    pub fn new(position: glam::Vec3, color: Color, intensity: f32) -> Self {
        Self {
            position,
            color,
            intensity,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct PointLightUniform {
    pub position: [f32; 4],
    pub color: [f32; 4],
    pub intensity: f32,
    _pad: [f32; 3],
}

impl From<&PointLight> for PointLightUniform {
    fn from(light: &PointLight) -> Self {
        Self {
            position: [light.position.x, light.position.y, light.position.z, 1.0],
            color: [light.color.r, light.color.g, light.color.b, 1.0],
            intensity: light.intensity,
            _pad: [0.0; 3],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct PointLightBuffer {
    pub lights: [PointLightUniform; MAX_LIGHTS],
    pub count: u32,
    _pad: [u32; 3],
}

impl PointLightBuffer {
    pub fn push(&mut self, light: PointLightUniform) {
        self.lights[self.count as usize] = light;
        self.count += 1;
    }

    pub fn clear(&mut self) {
        self.count = 0;
    }
}

impl From<&[PointLight]> for PointLightBuffer {
    fn from(lights: &[PointLight]) -> Self {
        let mut buffer = Self::default();
        for light in lights {
            buffer.push(light.into());
        }
        buffer
    }
}

#[derive(Debug, Clone, Copy, Component)]
pub struct DirectionalLight {
    pub direction: glam::Vec3,
    pub color: Color,
    pub intensity: f32,
}

impl DirectionalLight {
    pub fn new(direction: glam::Vec3, color: Color, intensity: f32) -> Self {
        Self {
            direction,
            color,
            intensity,
        }
    }

    pub fn view_transform(&self) -> glam::Mat4 {
        glam::Mat4::look_at_rh(
            glam::Vec3::ZERO,
            glam::Vec3::new(self.direction.x, self.direction.y, self.direction.z),
            glam::Vec3::Y,
        )
    }

    pub fn projection_transform(&self) -> glam::Mat4 {
        let left = -80.0;
        let right = 80.0;
        let bottom = -80.0;
        let top = 80.0;
        let near = -200.0;
        let far = 300.0;
        glam::Mat4::orthographic_rh(left, right, bottom, top, near, far)
    }
}

#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct DirectionalLightUniform {
    pub direction: [f32; 4],
    pub color: [f32; 4],
    pub view_transform: glam::Mat4,
    pub projection_transform: glam::Mat4,
    pub intensity: f32,
    _pad: [f32; 3],
}

impl From<&DirectionalLight> for DirectionalLightUniform {
    fn from(light: &DirectionalLight) -> Self {
        Self {
            direction: [light.direction.x, light.direction.y, light.direction.z, 0.0],
            color: [light.color.r, light.color.g, light.color.b, 1.0],
            view_transform: light.view_transform(),
            projection_transform: light.projection_transform(),
            intensity: light.intensity,
            _pad: [0.0; 3],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct DirectionalLightBuffer {
    pub lights: [DirectionalLightUniform; MAX_LIGHTS],
    pub count: u32,
    _pad: [u32; 3],
}

impl DirectionalLightBuffer {
    pub fn push(&mut self, light: DirectionalLightUniform) {
        self.lights[self.count as usize] = light;
        self.count += 1;
    }

    pub fn clear(&mut self) {
        self.count = 0;
    }
}

impl From<&[DirectionalLight]> for DirectionalLightBuffer {
    fn from(lights: &[DirectionalLight]) -> Self {
        let mut buffer = Self::default();
        for light in lights {
            buffer.push(light.into());
        }
        buffer
    }
}
