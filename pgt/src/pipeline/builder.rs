use super::depth_stencil::DepthStencilPreset;

/// Fluent builder for constructing `wgpu::RenderPipeline` instances cleanly.
pub struct PipelineBuilder<'a> {
    label: Option<&'a str>,
    shader: &'a wgpu::ShaderModule,
    vs_entry: &'a str,
    fs_entry: &'a str,
    layout: Option<&'a wgpu::PipelineLayout>,
    bind_group_layouts: Vec<&'a wgpu::BindGroupLayout>,
    targets: Vec<Option<wgpu::ColorTargetState>>,
    vertex_buffers: Vec<wgpu::VertexBufferLayout<'a>>,
    topology: wgpu::PrimitiveTopology,
    front_face: wgpu::FrontFace,
    cull_mode: Option<wgpu::Face>,
    depth_stencil: DepthStencilPreset,
}

impl<'a> PipelineBuilder<'a> {
    /// Start building a pipeline with a label and a compiled shader module.
    pub fn new(label: &'a str, shader: &'a wgpu::ShaderModule) -> Self {
        Self {
            label: Some(label),
            shader,
            vs_entry: "vs_main",
            fs_entry: "fs_main",
            layout: None,
            bind_group_layouts: Vec::new(),
            targets: Vec::new(),
            vertex_buffers: Vec::new(),
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            depth_stencil: DepthStencilPreset::Disabled,
        }
    }

    /// Set an explicit pipeline layout.
    pub fn with_layout(mut self, layout: &'a wgpu::PipelineLayout) -> Self {
        self.layout = Some(layout);
        self
    }

    /// Automatically generate the pipeline layout from an ordered slice of bind group layouts.
    pub fn with_bind_group_layouts(mut self, layouts: &[&'a wgpu::BindGroupLayout]) -> Self {
        self.bind_group_layouts.extend_from_slice(layouts);
        self
    }

    /// Set vertex shader entry point name (default: "vs_main").
    pub fn with_vs_entry(mut self, entry: &'a str) -> Self {
        self.vs_entry = entry;
        self
    }

    /// Set fragment shader entry point name (default: "fs_main").
    pub fn with_fs_entry(mut self, entry: &'a str) -> Self {
        self.fs_entry = entry;
        self
    }

    /// Add a color target format with REPLACE blend state.
    pub fn with_target(mut self, format: wgpu::TextureFormat) -> Self {
        self.targets.push(Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        }));
        self
    }

    /// Add a color target with custom blend state.
    pub fn with_target_blend(mut self, format: wgpu::TextureFormat, blend: wgpu::BlendState) -> Self {
        self.targets.push(Some(wgpu::ColorTargetState {
            format,
            blend: Some(blend),
            write_mask: wgpu::ColorWrites::ALL,
        }));
        self
    }

    /// Add vertex buffer layout(s).
    pub fn with_vertex_buffers(mut self, buffers: &[wgpu::VertexBufferLayout<'a>]) -> Self {
        self.vertex_buffers.extend_from_slice(buffers);
        self
    }

    /// Set depth-stencil preset.
    pub fn with_depth_stencil(mut self, preset: DepthStencilPreset) -> Self {
        self.depth_stencil = preset;
        self
    }

    /// Convenient alias for `with_depth_stencil`.
    pub fn with_depth(self, preset: DepthStencilPreset) -> Self {
        self.with_depth_stencil(preset)
    }

    /// Set polygon face culling mode (default: `Some(Face::Back)`).
    pub fn with_cull_mode(mut self, cull_mode: Option<wgpu::Face>) -> Self {
        self.cull_mode = cull_mode;
        self
    }

    /// Set primitive topology (default: `TriangleList`).
    pub fn with_topology(mut self, topology: wgpu::PrimitiveTopology) -> Self {
        self.topology = topology;
        self
    }

    /// Build the render pipeline on the given device.
    pub fn build(self, device: &wgpu::Device) -> wgpu::RenderPipeline {
        let created_layout;
        let layout_ref = if let Some(layout) = self.layout {
            layout
        } else if !self.bind_group_layouts.is_empty() {
            let entries: Vec<Option<&wgpu::BindGroupLayout>> = self
                .bind_group_layouts
                .iter()
                .map(|&bgl| Some(bgl))
                .collect();
            created_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: self.label.map(|l| format!("{l} Layout")).as_deref(),
                bind_group_layouts: &entries,
                immediate_size: 0,
            });
            &created_layout
        } else {
            created_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: self.label.map(|l| format!("{l} Layout")).as_deref(),
                bind_group_layouts: &[],
                immediate_size: 0,
            });
            &created_layout
        };

        let vertex_buffer_options: Vec<Option<wgpu::VertexBufferLayout<'a>>> = self
            .vertex_buffers
            .into_iter()
            .map(Some)
            .collect();

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: self.label,
            layout: Some(layout_ref),
            vertex: wgpu::VertexState {
                module: self.shader,
                entry_point: Some(self.vs_entry),
                compilation_options: Default::default(),
                buffers: &vertex_buffer_options,
            },
            primitive: wgpu::PrimitiveState {
                topology: self.topology,
                strip_index_format: None,
                front_face: self.front_face,
                cull_mode: self.cull_mode,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: self.depth_stencil.to_wgpu_state(),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: self.shader,
                entry_point: Some(self.fs_entry),
                compilation_options: Default::default(),
                targets: &self.targets,
            }),
            multiview_mask: None,
            cache: None,
        })
    }
}
