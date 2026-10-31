/// Common depth-stencil configurations.
#[derive(Debug, Clone, Copy)]
pub enum DepthStencilPreset {
    /// Depth write enabled, compare Less (standard opaque 3D scene objects).
    Less(wgpu::TextureFormat),
    /// Depth write disabled, compare LessEqual (ideal for skybox passes at z = 1.0).
    LessEqualNoWrite(wgpu::TextureFormat),
    /// Depth write enabled, compare LessEqual.
    LessEqual(wgpu::TextureFormat),
    /// Depth test disabled completely.
    Disabled,
}

impl DepthStencilPreset {
    pub fn to_wgpu_state(self) -> Option<wgpu::DepthStencilState> {
        match self {
            Self::Less(format) => Some(wgpu::DepthStencilState {
                format,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            Self::LessEqualNoWrite(format) => Some(wgpu::DepthStencilState {
                format,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            Self::LessEqual(format) => Some(wgpu::DepthStencilState {
                format,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            Self::Disabled => None,
        }
    }
}
