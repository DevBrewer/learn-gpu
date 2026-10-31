use glam::{Mat4, Vec3};
use std::time::Duration;
use winit::keyboard::KeyCode;

use super::projection::Projection;
use super::uniform::CameraUniform;
use crate::core::input::InputState;

/// First-person / Free-look fly camera (WASD movement, Space/Shift vertical, mouse look).
#[derive(Debug, Clone)]
pub struct FlyCamera {
    pub position: Vec3,
    pub yaw: f32,   // in radians
    pub pitch: f32, // in radians
    pub speed: f32,
    pub sensitivity: f32,
}

impl FlyCamera {
    /// Create a new fly camera at `position` with initial horizontal (`yaw`) and vertical (`pitch`) angles.
    pub fn new(position: Vec3, yaw: f32, pitch: f32) -> Self {
        Self {
            position,
            yaw,
            pitch,
            speed: 5.0,
            sensitivity: 0.003,
        }
    }

    /// Process keyboard movement (WASD, Space, Shift) and mouse looking from `InputState`.
    pub fn handle_input(&mut self, input: &InputState, dt: Duration) {
        let dt_secs = dt.as_secs_f32();

        // 1. Mouse rotation (look around when right mouse or left mouse is held)
        if input.is_right_mouse_down() || input.is_left_mouse_down() {
            let (dx, dy) = input.cursor_delta();
            self.yaw += dx * self.sensitivity;
            self.pitch -= dy * self.sensitivity;

            let max_pitch = 89.0f32.to_radians();
            self.pitch = self.pitch.clamp(-max_pitch, max_pitch);
        }

        // 2. Keyboard translation on horizontal plane (XZ)
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let forward = Vec3::new(cos_yaw, 0.0, sin_yaw).normalize();
        let right = Vec3::new(-sin_yaw, 0.0, cos_yaw).normalize();

        if input.is_key_down(KeyCode::KeyW) || input.is_key_down(KeyCode::ArrowUp) {
            self.position += forward * self.speed * dt_secs;
        }
        if input.is_key_down(KeyCode::KeyS) || input.is_key_down(KeyCode::ArrowDown) {
            self.position -= forward * self.speed * dt_secs;
        }
        if input.is_key_down(KeyCode::KeyD) || input.is_key_down(KeyCode::ArrowRight) {
            self.position += right * self.speed * dt_secs;
        }
        if input.is_key_down(KeyCode::KeyA) || input.is_key_down(KeyCode::ArrowLeft) {
            self.position -= right * self.speed * dt_secs;
        }

        // 3. World vertical movement
        if input.is_key_down(KeyCode::Space) {
            self.position.y += self.speed * dt_secs;
        }
        if input.is_key_down(KeyCode::ShiftLeft) || input.is_key_down(KeyCode::ShiftRight) {
            self.position.y -= self.speed * dt_secs;
        }

        // 4. Scroll wheel to adjust movement speed
        let (_, scroll_y) = input.scroll_delta();
        if scroll_y.abs() > 0.001 {
            self.speed = (self.speed + scroll_y * 0.5).clamp(0.5, 50.0);
        }
    }

    /// Forward unit facing vector derived from yaw and pitch.
    pub fn forward(&self) -> Vec3 {
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        Vec3::new(cos_pitch * cos_yaw, sin_pitch, cos_pitch * sin_yaw).normalize()
    }

    /// Compute right-handed view matrix.
    pub fn calc_view_matrix(&self) -> Mat4 {
        glam::camera::rh::view::look_to_mat4(self.position, self.forward(), Vec3::Y)
    }

    /// Convert to GPU-ready `CameraUniform`.
    pub fn to_uniform(&self, projection: &Projection) -> CameraUniform {
        let mut uniform = CameraUniform::new();
        uniform.update(
            self.position,
            self.calc_view_matrix(),
            projection.calc_matrix(),
        );
        uniform
    }
}
