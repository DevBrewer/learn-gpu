use pgt::prelude::*;
use std::path::Path;

// ---------------------------------------------------------------------------
// Uniform Data Structure (WebGPU 16-byte aligned)
// ---------------------------------------------------------------------------
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EditorUniform {
    pub transform: [f32; 4],     // scale_x, scale_y, pan_x, pan_y
    pub brightness: f32,         // -1.0 to 1.0
    pub contrast: f32,           // 0.0 to 2.0
    pub saturation: f32,         // 0.0 to 2.0
    pub exposure: f32,           // 0.1 to 3.0
    pub hue: f32,                // -PI to PI
    pub warmth: f32,             // -1.0 to 1.0
    pub vignette: f32,           // 0.0 to 1.0
    pub split_pos: f32,          // -1.0 (disabled) or 0.0..1.0
    pub invert: u32,             // 0 or 1
    pub grayscale: u32,          // 0 or 1
    pub sepia: u32,              // 0 or 1
    pub aspect_ratio: f32,       // image_aspect / viewport_aspect
}

impl Default for EditorUniform {
    fn default() -> Self {
        Self {
            transform: [1.0, 1.0, 0.0, 0.0],
            brightness: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            exposure: 1.0,
            hue: 0.0,
            warmth: 0.0,
            vignette: 0.0,
            split_pos: -1.0,
            invert: 0,
            grayscale: 0,
            sepia: 0,
            aspect_ratio: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Image Editor Application
// ---------------------------------------------------------------------------
pub struct ImageEditorApp {
    // Current Image Asset
    image_name: String,
    texture: Texture2D,
    texture_layout: wgpu::BindGroupLayout,
    texture_bind_group: wgpu::BindGroup,
    pending_image: Option<String>,

    // Rendering Pipeline & Uniforms
    pipeline: wgpu::RenderPipeline,
    uniform_binding: UniformBinding<EditorUniform>,
    dirty: bool,

    // Color Grading Adjustments
    brightness: f32,
    contrast: f32,
    saturation: f32,
    exposure: f32,
    hue_degrees: f32,
    warmth: f32,
    vignette: f32,
    invert: bool,
    grayscale: bool,
    sepia: bool,

    // A/B Comparison Split Screen
    split_view: bool,
    split_pos: f32,

    // Canvas Navigation
    zoom: f32,
    pan: (f32, f32),

    // Status & Path Inputs
    custom_path: String,
    status_message: String,
}

impl ImageEditorApp {
    fn reset_adjustments(&mut self) {
        self.brightness = 0.0;
        self.contrast = 1.0;
        self.saturation = 1.0;
        self.exposure = 1.0;
        self.hue_degrees = 0.0;
        self.warmth = 0.0;
        self.vignette = 0.0;
        self.invert = false;
        self.grayscale = false;
        self.sepia = false;
        self.dirty = true;
    }

    fn apply_preset(&mut self, preset: &str) {
        self.reset_adjustments();
        match preset {
            "Vibrant" => {
                self.saturation = 1.45;
                self.contrast = 1.2;
                self.warmth = 0.1;
                self.exposure = 1.05;
            }
            "Vintage Sepia" => {
                self.sepia = true;
                self.contrast = 1.15;
                self.vignette = 0.45;
                self.warmth = 0.25;
            }
            "Cyberpunk" => {
                self.hue_degrees = 68.0;
                self.contrast = 1.35;
                self.saturation = 1.6;
                self.warmth = -0.3;
            }
            "Noir (B&W)" => {
                self.grayscale = true;
                self.contrast = 1.45;
                self.brightness = -0.05;
                self.vignette = 0.5;
            }
            "Warm Sunset" => {
                self.warmth = 0.55;
                self.saturation = 1.3;
                self.contrast = 1.1;
                self.exposure = 1.05;
            }
            _ => {}
        }
        self.dirty = true;
    }

    fn load_image_from_path(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &str,
    ) -> anyhow::Result<()> {
        let bytes = std::fs::read(path)?;
        let texture = Texture2D::from_bytes(device, queue, &bytes, path, false)?;
        let bind_group = texture.create_bind_group(device, &self.texture_layout);

        self.texture = texture;
        self.texture_bind_group = bind_group;
        self.image_name = path.to_string();
        self.zoom = 1.0;
        self.pan = (0.0, 0.0);
        self.dirty = true;
        self.status_message = format!(
            "Loaded {} ({} × {} px)",
            path, self.texture.width, self.texture.height
        );
        Ok(())
    }

    fn build_uniform(&self, screen_w: u32, screen_h: u32) -> EditorUniform {
        let screen_aspect = screen_w as f32 / screen_h.max(1) as f32;
        let img_aspect = self.texture.width as f32 / self.texture.height.max(1) as f32;
        let aspect_ratio = img_aspect / screen_aspect;

        EditorUniform {
            transform: [self.zoom, self.zoom, self.pan.0, self.pan.1],
            brightness: self.brightness,
            contrast: self.contrast,
            saturation: self.saturation,
            exposure: self.exposure,
            hue: self.hue_degrees.to_radians(),
            warmth: self.warmth,
            vignette: self.vignette,
            split_pos: if self.split_view { self.split_pos } else { -1.0 },
            invert: if self.invert { 1 } else { 0 },
            grayscale: if self.grayscale { 1 } else { 0 },
            sepia: if self.sepia { 1 } else { 0 },
            aspect_ratio,
        }
    }

    fn save_to_file(&self, output_path: &str) -> anyhow::Result<()> {
        if !Path::new(&self.image_name).exists() {
            anyhow::bail!("Source image file '{}' not found", self.image_name);
        }

        let mut img = image::open(&self.image_name)?;

        // Apply brightness and contrast on CPU for export
        if (self.contrast - 1.0).abs() > 0.01 {
            img = img.adjust_contrast(self.contrast * 10.0 - 10.0);
        }
        if self.brightness.abs() > 0.01 {
            let b_int = (self.brightness * 100.0) as i32;
            img = img.brighten(b_int);
        }
        if self.grayscale {
            img = img.grayscale();
        }
        if self.invert {
            img.invert();
        }

        img.save(output_path)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// App Trait Implementation
// ---------------------------------------------------------------------------
impl pgt::App for ImageEditorApp {
    fn title() -> &'static str {
        "PGT Studio: Image Editor MVP"
    }

    fn init(ctx: &pgt::prelude::Context) -> anyhow::Result<Self> {
        // 1. Initial Image Loading
        let initial_path = "assets/happy-tree.png";
        let bytes = match std::fs::read(initial_path) {
            Ok(b) => b,
            Err(_) => {
                // Fallback to relative path or white pixel if file not located
                include_bytes!("../../../assets/happy-tree.png").to_vec()
            }
        };

        let texture_layout = Texture2D::create_bind_group_layout(&ctx.device);
        let texture = Texture2D::from_bytes(
            &ctx.device,
            &ctx.queue,
            &bytes,
            "Initial Image",
            false,
        )?;
        let texture_bind_group = texture.create_bind_group(&ctx.device, &texture_layout);

        // 2. Setup Uniform Binding using pgt::UniformBinding
        let initial_uniform = EditorUniform::default();
        let uniform_binding = UniformBinding::new(&ctx.device, "Editor Uniform", &initial_uniform);

        // 3. Compile WGSL Shader
        let shader = ctx.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Image Editor Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // 4. Build Render Pipeline using pgt::PipelineBuilder
        let pipeline = PipelineBuilder::new("Image Editor Pipeline", &shader)
            .with_bind_group_layouts(&[&texture_layout, &uniform_binding.layout])
            .with_target(ctx.config.format)
            .with_cull_mode(None)
            .build(&ctx.device);

        Ok(Self {
            image_name: initial_path.to_string(),
            texture,
            texture_layout,
            texture_bind_group,
            pending_image: None,
            pipeline,
            uniform_binding,
            dirty: true,

            brightness: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            exposure: 1.0,
            hue_degrees: 0.0,
            warmth: 0.0,
            vignette: 0.0,
            invert: false,
            grayscale: false,
            sepia: false,

            split_view: false,
            split_pos: 0.5,

            zoom: 1.0,
            pan: (0.0, 0.0),

            custom_path: "assets/happy-tree-cartoon.png".to_string(),
            status_message: "Ready. Pan with Right Drag or Space+Drag. Zoom with Mouse Wheel.".to_string(),
        })
    }

    fn update(&mut self, ctx: &mut pgt::prelude::Context, _dt: std::time::Duration) {
        // Load pending image if queued
        if let Some(path) = self.pending_image.take() {
            if let Err(e) = self.load_image_from_path(&ctx.device, &ctx.queue, &path) {
                self.status_message = format!("Error loading image: {e}");
            }
        }

        // Pan the image when holding Right Click or Space+Left Click
        let is_panning = ctx.input.is_right_mouse_down()
            || (ctx.input.is_left_mouse_down() && ctx.input.is_key_down(winit::keyboard::KeyCode::Space));

        if is_panning {
            let (dx, dy) = ctx.input.cursor_delta();
            if dx.abs() > 0.0 || dy.abs() > 0.0 {
                self.pan.0 += dx * 0.002 / self.zoom;
                self.pan.1 -= dy * 0.002 / self.zoom;
                self.dirty = true;
            }
        }

        // Zoom with mouse scroll wheel
        let (_, scroll_y) = ctx.input.scroll_delta();
        if scroll_y.abs() > 0.01 {
            let factor = 1.0 + scroll_y * 0.08;
            self.zoom = (self.zoom * factor).clamp(0.1, 15.0);
            self.dirty = true;
        }

        // Update GPU uniform buffer when values change
        if self.dirty {
            let uniform = self.build_uniform(ctx.width(), ctx.height());
            self.uniform_binding.update(&ctx.queue, &uniform);
            self.dirty = false;
        }
    }

    fn resize(&mut self, _ctx: &mut pgt::prelude::Context, _width: u32, _height: u32) {
        self.dirty = true;
    }

    fn file_dropped(&mut self, _ctx: &mut pgt::prelude::Context, path: std::path::PathBuf) {
        self.pending_image = Some(path.to_string_lossy().to_string());
    }

    fn render(
        &mut self,
        ctx: &pgt::prelude::Context,
        view: &wgpu::TextureView,
    ) -> anyhow::Result<()> {
        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Image Editor Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Editor Image Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Dark neutral canvas checkerboard background
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.08,
                            g: 0.08,
                            b: 0.10,
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

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.texture_bind_group, &[]);
            render_pass.set_bind_group(1, &self.uniform_binding.bind_group, &[]);
            render_pass.draw(0..6, 0..1);
        }

        ctx.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        // Theme & styling
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(18, 18, 22);
        visuals.window_corner_radius = egui::CornerRadius::same(8);
        ui.set_visuals(visuals);

        egui::Panel::left("editor_sidebar")
            .default_size(320.0)
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.heading("PGT Image Studio");
                ui.label(
                    egui::RichText::new("Real-Time WGPU GPU Post-Processing")
                        .color(egui::Color32::from_rgb(150, 150, 170))
                        .small(),
                );
                ui.separator();

                // -----------------------------------------------------------
                // Section 1: Image Assets & Native File Picker
                // -----------------------------------------------------------
                egui::CollapsingHeader::new("Image Assets")
                    .default_open(true)
                    .show(ui, |ui| {
                        let short_name = Path::new(&self.image_name)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(&self.image_name);

                        ui.horizontal(|ui| {
                            ui.label("Current:");
                            ui.strong(short_name);
                        });
                        ui.label(format!("Resolution: {} × {} px", self.texture.width, self.texture.height));

                        ui.add_space(6.0);

                        // Native File Dialog with rfd
                        if ui.button("Open Image File...").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Image Files", &["png", "jpg", "jpeg", "webp", "bmp", "hdr"])
                                .pick_file()
                            {
                                self.pending_image = Some(path.to_string_lossy().to_string());
                            }
                        }

                        ui.label(
                            egui::RichText::new("(Or drag and drop any image directly onto this window)")
                                .color(egui::Color32::from_rgb(140, 140, 160))
                                .small(),
                        );

                        ui.add_space(4.0);
                        ui.label("Sample Images:");
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Tree").clicked() {
                                self.pending_image = Some("assets/happy-tree.png".to_string());
                            }
                            if ui.button("Cartoon Tree").clicked() {
                                self.pending_image = Some("assets/happy-tree-cartoon.png".to_string());
                            }
                        });

                        ui.add_space(4.0);
                        ui.label("File Path:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.custom_path);
                            if ui.button("Load").clicked() {
                                self.pending_image = Some(self.custom_path.clone());
                            }
                        });
                    });

                ui.separator();

                // -----------------------------------------------------------
                // Section 2: One-Click Filter Presets
                // -----------------------------------------------------------
                egui::CollapsingHeader::new("Filter Presets")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Original").clicked() {
                                self.reset_adjustments();
                            }
                            if ui.button("Vibrant").clicked() {
                                self.apply_preset("Vibrant");
                            }
                            if ui.button("Vintage Sepia").clicked() {
                                self.apply_preset("Vintage Sepia");
                            }
                            if ui.button("Cyberpunk").clicked() {
                                self.apply_preset("Cyberpunk");
                            }
                            if ui.button("Noir B&W").clicked() {
                                self.apply_preset("Noir (B&W)");
                            }
                            if ui.button("Sunset Glow").clicked() {
                                self.apply_preset("Warm Sunset");
                            }
                        });
                    });

                ui.separator();

                // -----------------------------------------------------------
                // Section 3: Color Grading Sliders
                // -----------------------------------------------------------
                egui::CollapsingHeader::new("Tone & Color Adjustments")
                    .default_open(true)
                    .show(ui, |ui| {
                        let mut changed = false;

                        changed |= ui
                            .add(egui::Slider::new(&mut self.exposure, 0.2..=3.0).text("Exposure"))
                            .changed();
                        changed |= ui
                            .add(egui::Slider::new(&mut self.brightness, -1.0..=1.0).text("Brightness"))
                            .changed();
                        changed |= ui
                            .add(egui::Slider::new(&mut self.contrast, 0.0..=2.5).text("Contrast"))
                            .changed();
                        changed |= ui
                            .add(egui::Slider::new(&mut self.saturation, 0.0..=2.5).text("Saturation"))
                            .changed();
                        changed |= ui
                            .add(egui::Slider::new(&mut self.hue_degrees, -180.0..=180.0).text("Hue Shift"))
                            .changed();
                        changed |= ui
                            .add(egui::Slider::new(&mut self.warmth, -1.0..=1.0).text("Warmth / Temp"))
                            .changed();
                        changed |= ui
                            .add(egui::Slider::new(&mut self.vignette, 0.0..=1.0).text("Vignette"))
                            .changed();

                        ui.add_space(6.0);
                        ui.label("Creative Modes:");
                        ui.horizontal(|ui| {
                            changed |= ui.checkbox(&mut self.grayscale, "B&W").changed();
                            changed |= ui.checkbox(&mut self.sepia, "Sepia").changed();
                            changed |= ui.checkbox(&mut self.invert, "Invert").changed();
                        });

                        if changed {
                            self.dirty = true;
                        }

                        ui.add_space(6.0);
                        if ui.button("Reset Sliders").clicked() {
                            self.reset_adjustments();
                        }
                    });

                ui.separator();

                // -----------------------------------------------------------
                // Section 4: A/B Split Comparison View
                // -----------------------------------------------------------
                egui::CollapsingHeader::new("Compare (Before / After)")
                    .default_open(true)
                    .show(ui, |ui| {
                        if ui.checkbox(&mut self.split_view, "Enable Split Screen").changed() {
                            self.dirty = true;
                        }

                        if self.split_view {
                            ui.label("Split Divider Position:");
                            if ui.add(egui::Slider::new(&mut self.split_pos, 0.0..=1.0).text("Divider")).changed() {
                                self.dirty = true;
                            }
                            ui.label(
                                egui::RichText::new("Left: Original | Right: Edited")
                                    .color(egui::Color32::from_rgb(180, 180, 190))
                                    .small(),
                            );
                        }
                    });

                ui.separator();

                // -----------------------------------------------------------
                // Section 5: Canvas Viewport (Zoom & Pan)
                // -----------------------------------------------------------
                egui::CollapsingHeader::new("Canvas Viewport")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(format!("Zoom: {:.0}%", self.zoom * 100.0));
                            if ui.button("Fit").clicked() {
                                self.zoom = 1.0;
                                self.pan = (0.0, 0.0);
                                self.dirty = true;
                            }
                            if ui.button("1:1").clicked() {
                                self.zoom = 1.0;
                                self.pan = (0.0, 0.0);
                                self.dirty = true;
                            }
                        });

                        if ui.add(egui::Slider::new(&mut self.zoom, 0.1..=5.0).text("Zoom")).changed() {
                            self.dirty = true;
                        }

                        ui.label(format!("Pan: ({:.2}, {:.2})", self.pan.0, self.pan.1));
                        if ui.button("Center Image").clicked() {
                            self.pan = (0.0, 0.0);
                            self.dirty = true;
                        }

                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Shortcut: Right Click + Drag to pan; Mouse wheel to zoom.")
                                .color(egui::Color32::GRAY)
                                .small(),
                        );
                    });

                ui.separator();

                // -----------------------------------------------------------
                // Section 6: Export & Save
                // -----------------------------------------------------------
                egui::CollapsingHeader::new("Export & Output")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Save As...").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .set_file_name("output.png")
                                    .add_filter("PNG Image", &["png"])
                                    .save_file()
                                {
                                    match self.save_to_file(&path.to_string_lossy()) {
                                        Ok(_) => self.status_message = format!("Saved image to {:?}", path),
                                        Err(e) => self.status_message = format!("Export failed: {e}"),
                                    }
                                }
                            }

                            if ui.button("Quick Save (output.png)").clicked() {
                                match self.save_to_file("output.png") {
                                    Ok(_) => self.status_message = "Saved modified image to 'output.png'".to_string(),
                                    Err(e) => self.status_message = format!("Export failed: {e}"),
                                }
                            }
                        });
                    });

                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(&self.status_message)
                        .color(egui::Color32::from_rgb(120, 200, 140))
                        .small(),
                );
            });
    }
}

fn main() -> anyhow::Result<()> {
    pgt::run::<ImageEditorApp>("PGT Studio: Image Editor MVP", 1360, 800)
}
