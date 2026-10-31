use glam::Mat4;

/// Perspective projection configuration targeting WebGPU depth clip space [0, 1].
#[derive(Debug, Clone, Copy)]
pub struct Projection {
    pub aspect_ratio: f32,
    pub fovy: f32, // in radians
    pub znear: f32,
    pub zfar: f32,
}

impl Projection {
    pub fn new(width: u32, height: u32, fovy_degrees: f32, znear: f32, zfar: f32) -> Self {
        Self {
            aspect_ratio: width as f32 / height.max(1) as f32,
            fovy: fovy_degrees.to_radians(),
            znear,
            zfar,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if height > 0 {
            self.aspect_ratio = width as f32 / height as f32;
        }
    }

    /// Calculate right-handed perspective projection matrix for WebGPU [0, 1] depth.
    pub fn calc_matrix(&self) -> Mat4 {
        glam::camera::rh::proj::directx::perspective(
            self.fovy,
            self.aspect_ratio,
            self.znear,
            self.zfar,
        )
    }
}
