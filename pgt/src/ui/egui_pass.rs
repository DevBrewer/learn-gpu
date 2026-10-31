use std::sync::Arc;
use egui_wgpu::{Renderer, RendererOptions, ScreenDescriptor};
use egui_winit::{EventResponse, State as EguiWinitState};
use winit::{event::WindowEvent, window::Window};
use crate::core::Context;

/// Integrated GUI overlay pass powered by `egui-wgpu` and `egui-winit`.
pub struct EguiPass {
    pub egui_ctx: egui::Context,
    pub egui_winit_state: EguiWinitState,
    pub egui_renderer: Renderer,
}

impl EguiPass {
    /// Initialize egui context, winit event state, and WGPU renderer.
    pub fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat, window: &Arc<Window>) -> Self {
        let egui_ctx = egui::Context::default();
        let scale_factor = window.scale_factor() as f32;
        let egui_winit_state = EguiWinitState::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(scale_factor),
            None,
            None,
        );
        let egui_renderer = Renderer::new(device, output_format, RendererOptions::default());

        Self {
            egui_ctx,
            egui_winit_state,
            egui_renderer,
        }
    }

    /// Forward window event to egui-winit.
    pub fn handle_event(&mut self, window: &Window, event: &WindowEvent) -> EventResponse {
        self.egui_winit_state.on_window_event(window, event)
    }

    /// Execute the egui UI callback and render clipped shapes onto the provided target view.
    pub fn render<F>(&mut self, ctx: &Context, target_view: &wgpu::TextureView, mut run_ui: F)
    where
        F: FnMut(&mut egui::Ui),
    {
        let raw_input = self.egui_winit_state.take_egui_input(&ctx.window);
        let full_output = self.egui_ctx.run_ui(raw_input, |ui| {
            run_ui(ui);
        });

        self.egui_winit_state
            .handle_platform_output(&ctx.window, full_output.platform_output);

        let mut textures_delta = full_output.textures_delta;

        // Update texture deltas
        for (id, deltas) in textures_delta.set.drain() {
            for delta in deltas {
                self.egui_renderer
                    .update_texture(&ctx.device, &ctx.queue, id, &delta);
            }
        }

        let clipped_primitives = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [ctx.config.width, ctx.config.height],
            pixels_per_point: full_output.pixels_per_point,
        };

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("egui_overlay_encoder"),
            });

        let mut user_cmd_bufs = self.egui_renderer.update_buffers(
            &ctx.device,
            &ctx.queue,
            &mut encoder,
            &clipped_primitives,
            &screen_descriptor,
        );

        {
            let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui_overlay_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            let mut rpass = rpass.forget_lifetime();
            self.egui_renderer
                .render(&mut rpass, &clipped_primitives, &screen_descriptor);
        }

        // Free textures
        for id in textures_delta.free.drain() {
            self.egui_renderer.free_texture(&id);
        }

        user_cmd_bufs.push(encoder.finish());
        ctx.queue.submit(user_cmd_bufs);
    }
}
