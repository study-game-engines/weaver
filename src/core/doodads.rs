use weaver_proc_macro::Resource;

use crate::core::color::Color;

pub const MAX_DOODADS: usize = 100;

#[derive(Default, Resource)]
pub struct Doodads {
    pub doodads: Vec<Doodad>,
    pub locked: bool,
}

impl Doodads {
    pub fn new() -> Self {
        Self {
            doodads: Vec::new(),
            locked: false,
        }
    }

    pub fn lock(&mut self) {
        self.locked = true;
    }

    pub fn unlock(&mut self) {
        self.locked = false;
    }

    pub fn push(&mut self, doodad: Doodad) {
        if !self.locked && self.doodads.len() < MAX_DOODADS {
            self.doodads.push(doodad);
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Doodad {
    Cube(Cube),
    Cone(Cone),
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cube {
    pub position: glam::Vec3,
    pub rotation: glam::Quat,
    pub scale: glam::Vec3,
    pub color: Color,
}

impl Cube {
    pub fn new(
        position: glam::Vec3,
        rotation: glam::Quat,
        scale: glam::Vec3,
        color: Color,
    ) -> Self {
        Self {
            position,
            rotation,
            scale,
            color,
        }
    }
}

impl Default for Cube {
    fn default() -> Self {
        Self {
            position: glam::Vec3::ZERO,
            rotation: glam::Quat::IDENTITY,
            scale: glam::Vec3::ONE,
            color: Color::WHITE,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cone {
    pub position: glam::Vec3,
    pub rotation: glam::Quat,
    pub scale: glam::Vec3,
    pub color: Color,
}

impl Cone {
    pub fn new(
        position: glam::Vec3,
        rotation: glam::Quat,
        scale: glam::Vec3,
        color: Color,
    ) -> Self {
        Self {
            position,
            rotation,
            scale,
            color,
        }
    }
}

impl Default for Cone {
    fn default() -> Self {
        Self {
            position: glam::Vec3::ZERO,
            rotation: glam::Quat::IDENTITY,
            scale: glam::Vec3::ONE,
            color: Color::WHITE,
        }
    }
}
