// Core ECS components for the engine
// These are reusable across any game built with this engine

use bevy_ecs::prelude::*;
use glam::Vec3;

/// Position of an entity in 3D space
#[derive(Component, Debug, Clone, Copy)]
pub struct Transform {
    pub position: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
        }
    }
}

impl Transform {
    pub fn from_position(position: Vec3) -> Self {
        Self { position }
    }
}

/// RGB color for rendering
#[derive(Component, Debug, Clone, Copy)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

/// Velocity of an entity in 3D space (units per second)
#[derive(Component, Debug, Clone, Copy)]
pub struct Velocity {
    pub linear: Vec3,
}

/// Assigns an entity to a movement group.
/// The group_id indexes into the `groups` Vec stored on State.
#[derive(Component, Debug, Clone, Copy)]
pub struct GroupMembership {
    pub group_id: u32,
}
