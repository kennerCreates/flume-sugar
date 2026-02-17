// ECS systems for updating game state
// Systems operate on entities with specific component combinations

use bevy_ecs::prelude::*;
use glam::Vec3;
use super::components::*;

/// Update entity positions based on velocity
/// Runs every frame, applies velocity * delta_time to position
pub fn movement_system(
    mut query: Query<(&mut Transform, &Velocity)>,
    delta_time: f32,
) {
    for (mut transform, velocity) in query.iter_mut() {
        transform.position += velocity.linear * delta_time;
    }
}

/// Decrease lifetime and despawn entities when lifetime expires
pub fn lifetime_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Lifetime)>,
    delta_time: f32,
) {
    for (entity, mut lifetime) in query.iter_mut() {
        lifetime.remaining -= delta_time;
        if lifetime.remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// Keep entities within bounds (wrap around or bounce)
pub fn bounds_system(
    mut query: Query<&mut Transform>,
    bounds: Vec3,
) {
    let half_bounds = bounds / 2.0;

    for mut transform in query.iter_mut() {
        // Wrap around on X axis
        if transform.position.x > half_bounds.x {
            transform.position.x = -half_bounds.x;
        } else if transform.position.x < -half_bounds.x {
            transform.position.x = half_bounds.x;
        }

        // Wrap around on Z axis
        if transform.position.z > half_bounds.z {
            transform.position.z = -half_bounds.z;
        } else if transform.position.z < -half_bounds.z {
            transform.position.z = half_bounds.z;
        }

        // Clamp Y axis (keep above ground)
        if transform.position.y < 0.0 {
            transform.position.y = 0.0;
        } else if transform.position.y > bounds.y {
            transform.position.y = bounds.y;
        }
    }
}
