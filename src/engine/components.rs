// Core ECS components for the engine
// These are reusable across any game built with this engine

use bevy_ecs::prelude::*;
use glam::{Vec3, Quat};

/// Position, rotation, and scale of an entity in 3D space
#[derive(Component, Debug, Clone, Copy)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn from_position(position: Vec3) -> Self {
        Self {
            position,
            ..Default::default()
        }
    }

    pub fn with_scale(mut self, scale: Vec3) -> Self {
        self.scale = scale;
        self
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
    pub const RED: Self = Self { r: 1.0, g: 0.0, b: 0.0 };
    pub const GREEN: Self = Self { r: 0.0, g: 1.0, b: 0.0 };
    pub const BLUE: Self = Self { r: 0.0, g: 0.0, b: 1.0 };
    pub const YELLOW: Self = Self { r: 1.0, g: 1.0, b: 0.0 };
    pub const CYAN: Self = Self { r: 0.0, g: 1.0, b: 1.0 };
    pub const MAGENTA: Self = Self { r: 1.0, g: 0.0, b: 1.0 };
    pub const WHITE: Self = Self { r: 1.0, g: 1.0, b: 1.0 };

    pub fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    pub fn random() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        Self {
            r: rng.r#gen(),
            g: rng.r#gen(),
            b: rng.r#gen(),
        }
    }

    pub fn as_array(&self) -> [f32; 3] {
        [self.r, self.g, self.b]
    }
}

/// Entity despawns after this many seconds
#[derive(Component, Debug, Clone, Copy)]
pub struct Lifetime {
    pub remaining: f32,  // Seconds
}

impl Lifetime {
    pub fn new(seconds: f32) -> Self {
        Self { remaining: seconds }
    }
}
