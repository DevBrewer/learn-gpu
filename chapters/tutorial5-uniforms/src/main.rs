use framework::App;
use glam::{
    Mat4, Quat, Vec3,
    camera::rh::{proj::directx::perspective, view::look_at_mat4},
};
use wgpu::util::DeviceExt;

mod camera_controller;
mod texture;
pub use camera_controller::CameraController;
use winit::{event_loop::ActiveEventLoop, keyboard::KeyCode};

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                },
            ],
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [-0.0868241, 0.49240386, 0.0],
        tex_coords: [0.4131759, 0.00759614],
    }, // A
    Vertex {
        position: [-0.49513406, 0.06958647, 0.0],
        tex_coords: [0.0048659444, 0.43041354],
    }, // B
    Vertex {
        position: [-0.21918549, -0.44939706, 0.0],
        tex_coords: [0.28081453, 0.949397],
    }, // C
    Vertex {
        position: [0.35966998, -0.3473291, 0.0],
        tex_coords: [0.85967, 0.84732914],
    }, // D
    Vertex {
        position: [0.44147372, 0.2347359, 0.0],
        tex_coords: [0.9414737, 0.2652641],
    }, // E
];

const INDICES: &[u16] = &[0, 1, 4, 1, 2, 4, 2, 3, 4];

// Camera
pub struct Camera {
    eye: Vec3,
    target: Vec3,
    up: Vec3,
    aspect_ratio: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,
}

impl Camera {
    fn build_view_projection_matrix(&self) -> Mat4 {
        let view = look_at_mat4(self.eye, self.target, self.up);
        let proj = perspective(self.fovy, self.aspect_ratio, self.znear, self.zfar);

        proj * view
    }
}

// Camera Staging
struct CameraStaging {
    camera: Camera,
    model_translation: Vec3,
    model_rotation: Vec3,
    model_scale: Vec3,
}

impl CameraStaging {
    fn new(camera: Camera) -> Self {
        Self {
            camera,
            model_translation: Vec3::ZERO,
            model_rotation: Vec3::ZERO,
            model_scale: Vec3::ONE,
        }
    }

    fn build_model_matrix(&self) -> Mat4 {
        let rotation = Quat::from_euler(
            glam::EulerRot::XYZ,
            self.model_rotation.x,
            self.model_rotation.y,
            self.model_rotation.z,
        );
        Mat4::from_scale_rotation_translation(self.model_scale, rotation, self.model_translation)
    }

    fn update_camera(&self, camera_uniform: &mut CameraUniform) {
        let view_proj = self.camera.build_view_projection_matrix();
        let model = self.build_model_matrix();

        camera_uniform.model_view_proj = (view_proj * model).to_cols_array_2d();
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    model_view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    fn new() -> Self {
        Self {
            model_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        }
    }
}

struct State {
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    diffuse_bind_group: wgpu::BindGroup,
    camera_controller: CameraController,
    camera_staging: CameraStaging,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    // Model Translation: Arrow keys
    is_up_pressed: bool,
    is_down_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
    // Model Rotation: Q / E keys
    is_rot_ccw_pressed: bool,
    is_rot_cw_pressed: bool,
    // Model Scaling: + / - keys
    is_scale_up_pressed: bool,
    is_scale_down_pressed: bool,
}

impl App for State {
    fn title() -> &'static str {
        "Uniforms and Camera"
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        match code {
            KeyCode::Escape if is_pressed => event_loop.exit(),
            // Translation: Arrow Keys
            KeyCode::ArrowUp => self.is_up_pressed = is_pressed,
            KeyCode::ArrowDown => self.is_down_pressed = is_pressed,
            KeyCode::ArrowLeft => self.is_left_pressed = is_pressed,
            KeyCode::ArrowRight => self.is_right_pressed = is_pressed,
            // Rotation: Q / E Keys
            KeyCode::KeyQ => self.is_rot_ccw_pressed = is_pressed,
            KeyCode::KeyE => self.is_rot_cw_pressed = is_pressed,
            // Scaling: + / - Keys (Equal/Minus and Numpad)
            KeyCode::Equal | KeyCode::NumpadAdd => self.is_scale_up_pressed = is_pressed,
            KeyCode::Minus | KeyCode::NumpadSubtract => self.is_scale_down_pressed = is_pressed,
            // Camera: WASD Keys
            _ => {
                self.camera_controller.handle_key(code, is_pressed);
            }
        }
    }

    fn update(&mut self, ctx: &framework::Context) {
        self.camera_controller
            .update_camera(&mut self.camera_staging.camera);

        // Translation (Arrow keys: Up/Down on Y, Left/Right on X)
        let trans_speed = 0.02;
        if self.is_up_pressed {
            self.camera_staging.model_translation.y += trans_speed;
        }
        if self.is_down_pressed {
            self.camera_staging.model_translation.y -= trans_speed;
        }
        if self.is_left_pressed {
            self.camera_staging.model_translation.x -= trans_speed;
        }
        if self.is_right_pressed {
            self.camera_staging.model_translation.x += trans_speed;
        }

        // Rotation (Q: counter-clockwise, E: clockwise around Z axis)
        let rot_speed = 1.5_f32.to_radians();
        if self.is_rot_ccw_pressed {
            self.camera_staging.model_rotation.z += rot_speed;
        }
        if self.is_rot_cw_pressed {
            self.camera_staging.model_rotation.z -= rot_speed;
        }

        // Scaling (+ / =: scale up, -: scale down)
        let scale_speed = 0.02;
        if self.is_scale_up_pressed {
            self.camera_staging.model_scale += Vec3::splat(scale_speed);
        }
        if self.is_scale_down_pressed {
            self.camera_staging.model_scale =
                (self.camera_staging.model_scale - Vec3::splat(scale_speed)).max(Vec3::splat(0.05));
        }

        self.camera_staging.update_camera(&mut self.camera_uniform);

        ctx.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
    }

    fn init(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &wgpu::SurfaceConfiguration,
    ) -> anyhow::Result<Self> {
        let diffuse_bytes = include_bytes!("../../../assets/happy-tree.png");
        let diffuse_texture =
            texture::Texture::from_bytes(device, queue, diffuse_bytes, "happy tree")?;

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("diffuse_bind_group"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
        });
        // Camera
        let camera = Camera {
            eye: Vec3::new(0.0, 1.0, 2.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            aspect_ratio: config.width as f32 / config.height as f32,
            fovy: 45_f32.to_radians(),
            znear: 0.1,
            zfar: 100.0,
        };
        // Camera controller
        let camera_controller = CameraController::new(0.05);
        let mut camera_uniform = CameraUniform::new();
        let camera_staging = CameraStaging::new(camera);
        camera_staging.update_camera(&mut camera_uniform);

        // Camera Buffer
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Camera_bind_layout
        let camera_bind_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera Buffer layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        // Camera bind group
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // Vertex Buffer
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        // Index Buffer
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        // num_indices
        let num_indices = INDICES.len() as u32;

        // Load Shader Module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Uniform Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("./shader.wgsl").into()),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[Some(&texture_bind_group_layout), Some(&camera_bind_layout)],
                immediate_size: 0,
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(Vertex::desc())],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            render_pipeline,
            vertex_buffer,
            index_buffer,
            num_indices,
            diffuse_bind_group,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            camera_controller,
            camera_staging,
            is_up_pressed: false,
            is_down_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
            is_rot_ccw_pressed: false,
            is_rot_cw_pressed: false,
            is_scale_up_pressed: false,
            is_scale_down_pressed: false,
        })
    }

    fn render(&mut self, ctx: &framework::Context, view: &wgpu::TextureView) -> anyhow::Result<()> {
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Encoder Render"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.008,
                            g: 0.007,
                            b: 0.001,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
            render_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
        }

        ctx.queue.submit(std::iter::once(encoder.finish()));
        ctx.window.request_redraw();

        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    framework::run::<State>()
}
