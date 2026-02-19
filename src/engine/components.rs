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

/// Physical properties needed by the ORCA local-avoidance system (Sprint 2).
///
/// `radius`    — collision radius in world units.  Our procedural sphere mesh
///               has a diameter of 1.0, so 0.5 is the correct value.
/// `max_speed` — maximum speed in world units/sec (matches UNIT_SPEED).
#[derive(Component, Debug, Clone, Copy)]
pub struct UnitAgent {
    pub radius:    f32,
    pub max_speed: f32,
}

/// Fixed XZ offset from the group centroid assigned at spawn time.
///
/// The formation system uses this to compute each unit's slot target every
/// frame (`centroid + offset`) instead of dynamically reassigning slots.
/// Preserves the original spawn grid shape throughout the journey.
#[derive(Component, Debug, Clone, Copy)]
pub struct FormationOffset {
    pub offset: glam::Vec2,
}
