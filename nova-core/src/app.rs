use std::{cell::RefCell, rc::Rc, time::{Duration, Instant}};

use winit::{event::WindowEvent, event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, window::WindowAttributes};

mod handler;
mod builder;

pub use builder::ApplicationBuilder;

use crate::{EngineResult, assets::AssetsManager, errors::EngineError, graphics::{config::GraphicsConfiguration, context::GraphicsContext, frame::Frame, render::RenderContext}, time::Clock, window::WindowApi};

pub struct ApplicationContext {
    window_api: WindowApi,
    render_ctx: Rc<RefCell<RenderContext>>,
    pub assets_manager: AssetsManager,
}

impl ApplicationContext {
    fn request_window_redraw(&self) {
        self.window_api.window.request_redraw();
    }
}

pub trait ApplicationProxy {
    fn on_update(&mut self, ctx: &mut ApplicationContext, dt: Duration);
    fn on_render(&mut self, ctx: &ApplicationContext, frame: &mut Frame);
}

pub struct Application<P: ApplicationProxy> {
    window_attributes: WindowAttributes,
    gfx_config: GraphicsConfiguration,
    proxy: P,
    control_flow: ControlFlow,
    ctx: Option<ApplicationContext>,
    frame_clock: Clock,
    frame_time: Duration,

    engine_error: Option<EngineError>,
}

impl<P: ApplicationProxy> Application<P> {
    pub fn run(mut self) -> EngineResult<()> {
        let event_loop = EventLoop::new().unwrap();
        event_loop.set_control_flow(self.control_flow);

        event_loop.run_app(&mut self).unwrap();
        match self.engine_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn from_builder(builder: ApplicationBuilder<P>) -> Self {
        Self {
            window_attributes: builder.window_attributes,
            gfx_config: builder.gfx_config,
            proxy: builder.proxy,
            control_flow: builder.control_flow,
            ctx: None,
            frame_clock: Clock::new(),
            frame_time: Duration::from_millis(1000 / builder.frame_rate),
            engine_error: None,
        }
    }

    fn init(&mut self, event_loop: &ActiveEventLoop) -> EngineResult<()> {
        let window_visible= self.window_attributes.visible;

        let window_attributes = 
            self.window_attributes.clone()
            .with_visible(false)
        ;
        let window_api = WindowApi::new(event_loop, window_attributes)?;
        let gfx = GraphicsContext::new(window_api.window.clone(), self.gfx_config)?;
        let render_ctx = Rc::new(RefCell::new(RenderContext::new(gfx)));

        let assets_manager = AssetsManager::new(render_ctx.clone());

        window_api.window.set_visible(window_visible);

        self.ctx = Some(ApplicationContext {
            window_api,
            render_ctx,
            assets_manager,
        });

        Ok(())
    }

    fn process_events(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) -> EngineResult<()> {

        let ctx = self.ctx.as_mut().unwrap();
        let proxy = &mut self.proxy;

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::RedrawRequested => {
                Self::on_update(proxy, ctx, self.frame_time, self.frame_clock.restart());
                Self::on_render(proxy, ctx)?;

                if self.control_flow == ControlFlow::Poll {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + self.frame_time));
                }
            }

            WindowEvent::Resized(size) => {
                ctx.render_ctx.borrow().resize_surface(size.width, size.height);
            }

            _ => ()
        }

        Ok(())
    }

    fn on_update(proxy: &mut P, ctx: &mut ApplicationContext, frame_time: Duration, mut dt: Duration) {
        while dt >= frame_time {
            proxy.on_update(ctx, frame_time);
            dt -= frame_time;
        }
    }

    fn on_render(proxy: &mut P, ctx: &mut ApplicationContext) -> EngineResult<()> {
        let mut render_ctx = ctx.render_ctx.borrow_mut();
        let frame_opt= render_ctx.begin_frame()?;

        frame_opt.map(|mut frame| {
            proxy.on_render(ctx, &mut frame);
            frame.submit();
        });
        
        Ok(())
    }
}