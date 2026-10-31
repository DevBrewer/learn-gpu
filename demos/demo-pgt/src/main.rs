use pgt::prelude::*;
use std::time::Duration;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    pub position: [f32; 3],
    pub _pad0: u32,
    pub color: [f32; 3],
    pub _pad1: u32,
}

struct DemoApp {
    pipeline: wgpu::RenderPipeline,
    model: Model,
    camera: OrbitCamera,
    projection: Projection,
    camera_buffer: UniformBinding<CameraUniform>,
    light: LightUniform,
    light_pos: Vec3,
    light_buffer: UniformBinding<LightUniform>,
    depth_texture: DepthTexture,
    animate_light: bool,
    light_speed: f32,
    fps: f32,
    frame_count: u32,
    fps_timer: Duration,
}

impl App for DemoApp {
    fn title() -> &'static str {
        "pgt - 3D PBR Model Viewer & GUI Demo"
    }

    fn init(ctx: &Context) -> anyhow::Result<Self> {
        // 1. Camera & Perspective Projection
        let camera = OrbitCamera::new(Vec3::new(0.0, 1.8, 3.5), Vec3::ZERO);
        let projection = Projection::new(ctx.width(), ctx.height(), 45.0, 0.1, 100.0);
        let camera_buffer =
            UniformBinding::new(&ctx.device, "Camera", &camera.to_uniform(&projection));

        // 2. Point Light Uniform
        let light_pos = Vec3::new(2.0, 2.5, 2.0);
        let light = LightUniform {
            position: light_pos.to_array(),
            _pad0: 0,
            color: [1.0, 1.0, 0.95],
            _pad1: 0,
        };
        let light_buffer = UniformBinding::new(&ctx.device, "Light", &light);

        // 3. Load 3D Model and apply diffuse & normal maps if available
        let mut model = load_gltf(&ctx.device, &ctx.queue, "assets/cube.gltf")?;
        let material_layout = Material::create_layout(&ctx.device);

        if let (Ok(diffuse_bytes), Ok(normal_bytes)) = (
            std::fs::read("assets/cube-diffuse.jpg"),
            std::fs::read("assets/cube-normal.png"),
        ) {
            let diffuse = Texture2D::from_bytes(
                &ctx.device,
                &ctx.queue,
                &diffuse_bytes,
                "Cube Diffuse",
                true,
            )?;
            let normal = Texture2D::from_bytes(
                &ctx.device,
                &ctx.queue,
                &normal_bytes,
                "Cube Normal",
                false,
            )?;
            model.materials[0] =
                Material::new(&ctx.device, "CubeMat", diffuse, normal, &material_layout);
        }

        // 4. Depth Buffer Texture
        let depth_texture = DepthTexture::create(&ctx.device, ctx.width(), ctx.height(), "Depth");

        // 5. Construct Render Pipeline via Fluent Builder
        let shader = ctx
            .device
            .create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let pipeline = PipelineBuilder::new("Main Scene Pipeline", &shader)
            .with_bind_group_layouts(&[
                &material_layout,
                &camera_buffer.layout,
                &light_buffer.layout,
            ])
            .with_target(ctx.format())
            .with_depth_stencil(DepthStencilPreset::Less(DepthTexture::FORMAT))
            .with_vertex_buffers(&[ModelVertex::desc()])
            .build(&ctx.device);

        Ok(Self {
            pipeline,
            model,
            camera,
            projection,
            camera_buffer,
            light,
            light_pos,
            light_buffer,
            depth_texture,
            animate_light: true,
            light_speed: 1.0,
            fps: 60.0,
            frame_count: 0,
            fps_timer: Duration::ZERO,
        })
    }

    fn update(&mut self, ctx: &mut Context, dt: Duration) {
        // Track FPS
        self.frame_count += 1;
        self.fps_timer += dt;
        if self.fps_timer >= Duration::from_millis(500) {
            self.fps = self.frame_count as f32 / self.fps_timer.as_secs_f32();
            self.frame_count = 0;
            self.fps_timer = Duration::ZERO;
        }

        // Camera input handling
        self.camera.handle_input(&ctx.input);
        self.camera_buffer
            .update(&ctx.queue, &self.camera.to_uniform(&self.projection));

        // Animated light rotation
        if self.animate_light {
            let angle = dt.as_secs_f32() * self.light_speed;
            let rot = Quat::from_axis_angle(Vec3::Y, angle);
            self.light_pos = rot * self.light_pos;
            self.light.position = self.light_pos.to_array();
            self.light_buffer.update(&ctx.queue, &self.light);
        }
    }

    fn resize(&mut self, ctx: &mut Context, width: u32, height: u32) {
        self.projection.resize(width, height);
        self.depth_texture.resize(&ctx.device, width, height);
    }

    fn render(&mut self, ctx: &Context, view: &wgpu::TextureView) -> anyhow::Result<()> {
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Scene Command Encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("3D Forward Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.08,
                            g: 0.09,
                            b: 0.12,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            rpass.set_pipeline(&self.pipeline);
            rpass.draw_model_with_light(
                &self.model,
                &self.camera_buffer.bind_group,
                &self.light_buffer.bind_group,
            );
        }

        ctx.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        egui::Window::new("pgt Controls")
            .default_width(280.0)
            .show(ui.ctx(), |ui| {
                ui.heading("Render Performance");
                ui.label(format!("FPS: {:.1}", self.fps));
                ui.separator();

                ui.heading("Camera");
                ui.label(format!(
                    "Eye: [{:.2}, {:.2}, {:.2}]",
                    self.camera.eye_pos().x,
                    self.camera.eye_pos().y,
                    self.camera.eye_pos().z
                ));
                ui.label(format!("Distance: {:.2}", self.camera.distance));
                if ui.button("Reset Camera").clicked() {
                    self.camera = OrbitCamera::new(Vec3::new(0.0, 1.8, 3.5), Vec3::ZERO);
                }
                ui.separator();

                ui.heading("Lighting");
                ui.checkbox(&mut self.animate_light, "Animate Orbit");
                ui.add(egui::Slider::new(&mut self.light_speed, 0.0..=5.0).text("Orbit Speed"));
                let mut light_col = [
                    self.light.color[0],
                    self.light.color[1],
                    self.light.color[2],
                ];
                if ui.color_edit_button_rgb(&mut light_col).changed() {
                    self.light.color = light_col;
                }
                ui.separator();

                ui.collapsing("Controls Guide", |ui| {
                    ui.label("• Left Mouse Drag: Orbit Camera");
                    ui.label("• Scroll Wheel: Zoom In / Out");
                    ui.label("• Esc: Exit Application");
                });
            });
    }
}

fn main() -> anyhow::Result<()> {
    run::<DemoApp>("pgt Demo", 1280, 720)
}
