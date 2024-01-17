use weaver::prelude::*;

#[derive(Debug, Clone, Copy, Component)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Npc {
    pub speed: f32,
    pub rotation_speed: f32,
}

impl Default for Npc {
    fn default() -> Self {
        Self {
            speed: 10.0,
            rotation_speed: 1.0,
        }
    }
}
