use glam::{Mat4, Vec3};
use crate::core::input::InputState;
use super::projection::Projection;
use super::uniform::CameraUniform;

/// Orbit / Turntable camera centered around a target pivot point.
///
/// Ideal for 3D model viewers, product inspection, CAD, and virtual try-on avatars.
#[derive(Debug, Clone)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub sensitivity: f32,
    pub zoom_speed: f32,
}

impl OrbitCamera {
    /// Create a new orbit camera focusing on `target` from `initial_eye`.
    pub fn new(initial_eye: Vec3, target: Vec3) -> Self {
        let offset = initial_eye - target;
        let distance = offset.length().max(0.1);
        let pitch = (offset.y / distance).asin();
        let yaw = offset.z.atan2(offset.x);

        Self {
            target,
            yaw,
            pitch,
            distance,
            min_distance: 0.5,
            max_distance: 100.0,
            sensitivity: 0.005,
            zoom_speed: 0.5,
        }
    }

    /// Process keyboard & mouse input events from `InputState`.
    pub fn handle_input(&mut self, input: &InputState) {
        // Drag with left mouse button to rotate
        if input.is_left_mouse_down() {
            let (dx, dy) = input.cursor_delta();
            self.yaw += dx * self.sensitivity;
            self.pitch -= dy * self.sensitivity;

            // Clamp pitch to prevent gimbal flip
            let max_pitch = 89.0f32.to_radians();
            self.pitch = self.pitch.clamp(-max_pitch, max_pitch);
        }

        // Scroll wheel to zoom in/out
        let (_, scroll_y) = input.scroll_delta();
        if scroll_y.abs() > 0.001 {
            self.distance -= scroll_y * self.zoom_speed;
            self.distance = self.distance.clamp(self.min_distance, self.max_distance);
        }
    }

    /// Compute world-space position of camera eye.
    pub fn eye_pos(&self) -> Vec3 {
        let x = self.distance * self.pitch.cos() * self.yaw.cos();
        let y = self.distance * self.pitch.sin();
        let z = self.distance * self.pitch.cos() * self.yaw.sin();
        self.target + Vec3::new(x, y, z)
    }

    /// Compute right-handed view matrix.
    pub fn calc_view_matrix(&self) -> Mat4 {
        glam::camera::rh::view::look_at_mat4(self.eye_pos(), self.target, Vec3::Y)
    }

    /// Convert to GPU-ready `CameraUniform`.
    pub fn to_uniform(&self, projection: &Projection) -> CameraUniform {
        let mut uniform = CameraUniform::new();
        uniform.update(self.eye_pos(), self.calc_view_matrix(), projection.calc_matrix());
        uniform
    }
}
