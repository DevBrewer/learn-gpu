use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{WindowAttributes, WindowId},
};

use super::context::Context;
use super::input::InputState;

#[cfg(feature = "egui")]
use crate::ui::EguiPass;

/// The primary trait implemented by applications using `pgt`.
pub trait App: Sized + 'static {
    /// Window title.
    fn title() -> &'static str {
        "pgt application"
    }

    /// Initialization with graphics device, queue, and surface config.
    fn init(ctx: &Context) -> anyhow::Result<Self>;

    /// Per-frame update hook with elapsed delta time.
    fn update(&mut self, _ctx: &mut Context, _dt: Duration) {}

    /// Per-frame render hook targeting the swapchain texture view.
    fn render(&mut self, ctx: &Context, view: &wgpu::TextureView) -> anyhow::Result<()>;

    /// Surface resize notification.
    fn resize(&mut self, _ctx: &mut Context, _width: u32, _height: u32) {}

    /// Notification when a file is dragged and dropped onto the window.
    fn file_dropped(&mut self, _ctx: &mut Context, _path: std::path::PathBuf) {}

    /// Optional egui user interface overlay (active when `feature = "egui"` is enabled).
    ///
    /// Receives `&mut egui::Ui` directly representing the root viewport UI,
    /// so you can add panels or widgets without needing `ctx.root_ui`.
    #[cfg(feature = "egui")]
    fn render_ui(&mut self, _ui: &mut egui::Ui) {}
}

struct AppRunner<A: App> {
    title: &'static str,
    initial_size: (u32, u32),
    state: Option<RunningState<A>>,
}

/// The runtime state holding the context and application.
/// Declarative drop order: `egui_pass` and `app` drop first (releasing pipeline,
/// buffer, and texture references to `device`), followed by `ctx` (which drops
/// `device`, `surface`, `instance`, and `window` in strict reverse creation order).
struct RunningState<A: App> {
    #[cfg(feature = "egui")]
    egui_pass: EguiPass,
    app: A,
    last_frame: Instant,
    ctx: Context,
}

impl<A: App> ApplicationHandler for AppRunner<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = WindowAttributes::default()
            .with_title(self.title)
            .with_inner_size(PhysicalSize::new(self.initial_size.0, self.initial_size.1));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );
        let size = window.inner_size();

        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .expect("Failed to create surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: true,
        }))
        .expect("Failed to find appropriate GPU adapter");

        let required_limits = wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits());
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("pgt_primary_device"),
            required_features: wgpu::Features::empty(),
            required_limits,
            memory_hints: Default::default(),
            ..Default::default()
        }))
        .expect("Failed to request WGPU device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        let ctx = Context {
            config,
            input: InputState::new(),
            queue,
            device,
            surface,
            instance,
            window: window.clone(),
        };

        #[cfg(feature = "egui")]
        let egui_pass = EguiPass::new(&ctx.device, ctx.format(), &window);

        let app = A::init(&ctx).expect("Failed to initialize application");

        self.state = Some(RunningState {
            #[cfg(feature = "egui")]
            egui_pass,
            app,
            last_frame: Instant::now(),
            ctx,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else { return };

        #[cfg(feature = "egui")]
        {
            let response = state.egui_pass.handle_event(&state.ctx.window, &event);
            if response.consumed {
                state.ctx.window.request_redraw();
                return;
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                self.state.take();
                event_loop.exit();
                return;
            }
            WindowEvent::DroppedFile(path) => {
                state.app.file_dropped(&mut state.ctx, path);
                state.ctx.window.request_redraw();
            }
            WindowEvent::Resized(size) => {
                state.ctx.resize(size.width, size.height);
                state.app.resize(&mut state.ctx, size.width, size.height);
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.ctx.input.handle_cursor_moved(position);
            }
            WindowEvent::MouseInput {
                state: st, button, ..
            } => {
                state.ctx.input.handle_mouse_button(button, st);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                state.ctx.input.handle_mouse_wheel(delta);
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state: st,
                        ..
                    },
                ..
            } => {
                if key == KeyCode::Escape && st == ElementState::Pressed {
                    self.state.take();
                    event_loop.exit();
                    return;
                }
                state.ctx.input.handle_key(key, st);
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(state.last_frame);
                state.last_frame = now;

                state.app.update(&mut state.ctx, dt);

                match state.ctx.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame) => {
                        let view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());

                        // Render 3D scene
                        if let Err(e) = state.app.render(&state.ctx, &view) {
                            log::error!("Render Error: {e}");
                            self.state.take();
                            event_loop.exit();
                            return;
                        }

                        // Render GUI overlay if egui feature is active
                        #[cfg(feature = "egui")]
                        {
                            state.egui_pass.render(&state.ctx, &view, |ui| {
                                state.app.render_ui(ui);
                            });
                        }

                        state.ctx.queue.present(frame);
                    }
                    wgpu::CurrentSurfaceTexture::Outdated => {
                        state
                            .ctx
                            .surface
                            .configure(&state.ctx.device, &state.ctx.config);
                    }
                    _ => {}
                }

                state.ctx.input.end_frame();
            }
            _ => {}
        }

        if let Some(state) = &self.state {
            state.ctx.window.request_redraw();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.state.take();
    }
}

/// Run an application with the specified window title and initial dimensions.
pub fn run<A: App>(title: &'static str, width: u32, height: u32) -> anyhow::Result<()> {
    let _ = env_logger::try_init();
    let event_loop = EventLoop::new()?;
    let mut runner = AppRunner::<A> {
        title,
        initial_size: (width, height),
        state: None,
    };
    event_loop.run_app(&mut runner)?;
    Ok(())
}
