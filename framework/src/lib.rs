use std::sync::Arc;

use log::info;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

pub struct Context {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub mouse_down: bool,
}

impl Context {
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }
}

pub trait App: Sized {
    // Required
    fn title() -> &'static str;
    fn init(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &wgpu::SurfaceConfiguration,
    ) -> anyhow::Result<Self>;
    fn render(&mut self, ctx: &Context, view: &wgpu::TextureView) -> anyhow::Result<()>;

    // Optional hooks with default implementations
    fn resize(&mut self, _ctx: &mut Context, _width: u32, _height: u32) {}
    fn update(&mut self, _ctx: &Context) {}
    fn handle_key(&mut self, _event_loop: &ActiveEventLoop, _code: KeyCode, _is_pressed: bool) {}
    fn handle_mouse_move(&mut self, _x: f32, _y: f32) {}

    fn mouse_pressed(&mut self, _ctx: &Context, _button: MouseButton) {}
    fn mouse_release(&mut self, _ctx: &Context, _button: MouseButton) {}
    fn handle_mouse_wheel(&mut self, _delta: &MouseScrollDelta) {}
}

pub struct State<A: App> {
    ctx: Context,
    app: A,
}

impl<A: App> State<A> {
    async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });

        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptionsBase {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                apply_limit_buckets: true,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::wgt::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_formats = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_formats,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };

        let app = A::init(&device, &queue, &config)?;
        let ctx = Context {
            window,
            surface,
            device,
            queue,
            config,
            mouse_x: 0.0,
            mouse_y: 0.0,
            mouse_down: false,
        };

        Ok(Self { ctx, app })
    }
}

struct Runner<A: App> {
    state: Option<State<A>>,
}

impl<A: App> ApplicationHandler for Runner<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        info!("Event loop resumed. Creating window and GPU context..");

        let window_attributes = Window::default_attributes().with_title(A::title());
        let window = Arc::new(
            event_loop
                .create_window(window_attributes)
                .expect("Failed to create window."),
        );

        info!("GPU device and surface initialized successfully!");
        let state = pollster::block_on(State::new(window)).unwrap();
        self.state = Some(state);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = &mut self.state else { return };

        match event {
            WindowEvent::CloseRequested => {
                info!("Close requested. Exiting window..");
                event_loop.exit();
            }
            WindowEvent::Resized(sz) => {
                info!("Window resize to: {}x{}", sz.width, sz.height);
                state.ctx.resize(sz.width, sz.height);
                state.app.resize(&mut state.ctx, sz.width, sz.height);
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.ctx.mouse_x = position.x as f32;
                state.ctx.mouse_y = position.y as f32;

                state
                    .app
                    .handle_mouse_move(state.ctx.mouse_x, state.ctx.mouse_y);
            }
            WindowEvent::MouseInput {
                state: element_state,
                button,
                ..
            } => {
                let pressed = element_state.is_pressed();
                if button == MouseButton::Left {
                    state.ctx.mouse_down = pressed;
                }
                if pressed {
                    state.app.mouse_pressed(&state.ctx, button);
                } else {
                    state.app.mouse_release(&state.ctx, button);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                state.app.handle_mouse_wheel(&delta);
                state.ctx.window.request_redraw();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: key_state,
                        ..
                    },
                ..
            } => {
                state
                    .app
                    .handle_key(event_loop, code, key_state.is_pressed());
                state.ctx.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                state.app.update(&state.ctx);

                match state.ctx.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame) => {
                        let view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());

                        if let Err(e) = state.app.render(&state.ctx, &view) {
                            log::error!("Render Error: {e}");
                            event_loop.exit();
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
            }
            _ => {}
        }
        // Request the next frame to maintain a continuous render loop
        state.ctx.window.request_redraw();
    }
}

pub fn run<A: App + 'static>() -> anyhow::Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new()?;
    let mut runner = Runner::<A> { state: None };
    event_loop.run_app(&mut runner)?;
    Ok(())
}
