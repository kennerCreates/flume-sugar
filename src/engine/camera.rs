// RTS-style camera system
// See docs/research/camera-system.md for design rationale
//
// Camera model:
//   - A "target" point on the XZ ground plane (Y=0) that the camera looks at
//   - Fixed pitch (elevation angle) and yaw (horizontal rotation)
//   - Zoom by adjusting distance along the look vector
//   - WASD movement moves the target on XZ relative to camera facing direction
//   - Mouse wheel zooms in/out
//   - Edge scrolling moves target when mouse is near screen edges

use glam::{Mat4, Vec2, Vec3};
use super::input::InputState;
use winit::keyboard::KeyCode;

pub struct RtsCamera {
    /// Point on the ground plane (X/Z) the camera orbits around.
    /// Private: always clamped to bounds in update(). Use target() to read.
    target: Vec2,

    /// Distance from target along the look direction.
    /// Private: always clamped to [min_distance, max_distance] in update(). Use distance() to read.
    distance: f32,
    pub min_distance: f32,
    pub max_distance: f32,

    /// Elevation angle in radians (0 = horizontal, PI/2 = straight down)
    pub pitch: f32,

    /// Horizontal rotation in radians (0 = looking along -Z axis)
    pub yaw: f32,

    /// Vertical field of view in radians (low for RTS isometric feel)
    pub fov: f32,
    pub near: f32,
    pub far: f32,

    /// WASD pan speed in world units per second
    pub move_speed: f32,

    /// Zoom change (in distance units) per scroll line
    pub zoom_speed: f32,

    /// Edge scrolling speed in world units per second
    pub edge_scroll_speed: f32,

    /// How many pixels from the screen edge trigger edge scrolling
    pub edge_scroll_margin: f32,

    /// Map bounds: target is clamped to [bounds_min, bounds_max] on X/Z
    pub bounds_min: Vec2,
    pub bounds_max: Vec2,
}

impl RtsCamera {
    pub fn new() -> Self {
        Self {
            target: Vec2::ZERO,
            distance: 30.0,
            min_distance: 10.0,
            max_distance: 60.0,
            pitch: 55.0_f32.to_radians(),
            yaw: 0.0,
            fov: 20.0_f32.to_radians(),
            near: 0.1,
            far: 200.0,
            move_speed: 20.0,
            zoom_speed: 3.0,
            edge_scroll_speed: 15.0,
            edge_scroll_margin: 20.0,
            bounds_min: Vec2::new(-50.0, -50.0),
            bounds_max: Vec2::new(50.0, 50.0),
        }
    }

    /// Update camera position based on input. Call once per frame before rendering.
    pub fn update(&mut self, input: &InputState, dt: f32) {
        // Camera-relative movement directions on the XZ plane.
        // Forward (W) moves along camera facing projected onto XZ.
        // yaw=0 means camera faces along -Z, so forward is (0, -1) in (X, Z).
        let forward = Vec2::new(-self.yaw.sin(), -self.yaw.cos());
        let right = Vec2::new(self.yaw.cos(), -self.yaw.sin());

        let mut move_dir = Vec2::ZERO;

        if input.is_key_held(KeyCode::KeyW) { move_dir += forward; }
        if input.is_key_held(KeyCode::KeyS) { move_dir -= forward; }
        if input.is_key_held(KeyCode::KeyD) { move_dir += right; }
        if input.is_key_held(KeyCode::KeyA) { move_dir -= right; }

        if move_dir != Vec2::ZERO {
            self.target += move_dir.normalize() * self.move_speed * dt;
        }

        // Edge scrolling
        let (mx, my) = input.mouse_position;
        let (ww, wh) = (input.window_size.0 as f32, input.window_size.1 as f32);
        let m = self.edge_scroll_margin;

        if ww > 0.0 && wh > 0.0 {
            let mut edge_dir = Vec2::ZERO;

            if mx < m       { edge_dir -= right; }
            if mx > ww - m  { edge_dir += right; }
            if my < m       { edge_dir += forward; }
            if my > wh - m  { edge_dir -= forward; }

            if edge_dir != Vec2::ZERO {
                self.target += edge_dir.normalize() * self.edge_scroll_speed * dt;
            }
        }

        // Zoom: scroll up (positive delta) zooms in (decreases distance)
        self.distance -= input.scroll_delta * self.zoom_speed;
        self.distance = self.distance.clamp(self.min_distance, self.max_distance);

        // Clamp target to map bounds
        self.target = self.target.clamp(self.bounds_min, self.bounds_max);
    }

    /// World-space position of the camera eye.
    pub fn camera_position(&self) -> Vec3 {
        let target_3d = Vec3::new(self.target.x, 0.0, self.target.y);
        target_3d + self.eye_offset()
    }

    /// View matrix: looks from the camera eye toward the target.
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.camera_position(), Vec3::new(self.target.x, 0.0, self.target.y), Vec3::Y)
    }

    /// Perspective projection matrix.
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov, aspect, self.near, self.far)
    }

    /// Combined view-projection matrix ready to upload to the GPU.
    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        self.projection_matrix(aspect) * self.view_matrix()
    }

    pub fn target(&self) -> Vec2 { self.target }
    pub fn distance(&self) -> f32 { self.distance }

    /// Zoom fraction in [0, 1]: 1 = fully zoomed in (min_distance), 0 = fully zoomed out.
    /// Matches player intuition: zoom 100% = closest view.
    pub fn zoom_fraction(&self) -> f32 {
        let range = self.max_distance - self.min_distance;
        if range > 0.0 {
            1.0 - (self.distance - self.min_distance) / range
        } else {
            0.0
        }
    }

    // Offset from target to camera eye based on pitch, yaw, and distance.
    fn eye_offset(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos() * self.distance,
            self.pitch.sin() * self.distance,
            self.yaw.cos() * self.pitch.cos() * self.distance,
        )
    }
}
