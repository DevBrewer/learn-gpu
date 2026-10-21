use framework::App;

struct WindowApp;

impl App for WindowApp {
    fn title() -> &'static str {
        "Window Surface"
    }
    fn init(
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _config: &wgpu::SurfaceConfiguration,
    ) -> anyhow::Result<Self> {
        Ok(Self)
    }

    fn render(&mut self, ctx: &framework::Context, view: &wgpu::TextureView) -> anyhow::Result<()> {
        // Create a command encoder to clear the screen wit a nice color
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::wgt::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Encoder"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Clears the window to a dark blue-gray color
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
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
        }
        ctx.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    framework::run::<WindowApp>()
}
