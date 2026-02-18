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

/// Velocity for moving entities
#[derive(Component, Debug, Clone, Copy)]
pub struct Velocity {
    pub linear: Vec3,  // Units per second
}

impl Velocity {
    pub fn new(linear: Vec3) -> Self {
        Self { linear }
    }
}

/// RGB color for rendering
#[derive(Component, Debug, Clone, Copy)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color {
    pub fn random() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        Self {
            r: rng.r#gen(),
            g: rng.r#gen(),
            b: rng.r#gen(),
        }
    }
}

