use glam::{Mat4, Vec3};

/// Standard camera uniform layout mapped directly to GPU memory.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_pos: [f32; 4],
    pub view: [[f32; 4]; 4],
    pub view_proj: [[f32; 4]; 4],
    pub inv_proj: [[f32; 4]; 4],
    pub inv_view: [[f32; 4]; 4],
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self {
            view_pos: [0.0; 4],
            view: Mat4::IDENTITY.to_cols_array_2d(),
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            inv_proj: Mat4::IDENTITY.to_cols_array_2d(),
            inv_view: Mat4::IDENTITY.to_cols_array_2d(),
        }
    }
}

impl CameraUniform {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, eye: Vec3, view: Mat4, proj: Mat4) {
        self.view_pos = [eye.x, eye.y, eye.z, 1.0];
        self.view = view.to_cols_array_2d();
        self.view_proj = (proj * view).to_cols_array_2d();
        self.inv_proj = proj.inverse().to_cols_array_2d();
        self.inv_view = view.inverse().to_cols_array_2d();
    }
}
