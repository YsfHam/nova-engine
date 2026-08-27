
use std::time::{Duration, Instant};

use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::{ActiveEventLoop, ControlFlow}};

use crate::{EngineResult, app::{Application, ApplicationContext, ApplicationProxy}, graphics::render_target::RenderTarget};

impl<P: ApplicationProxy> ApplicationHandler for Application<P> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
       if let Err(error) = self.init(event_loop) {
            self.engine_error = Some(error);
            event_loop.exit();
       }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {

        if let Err(e) = self.process_events(event_loop, event) {
            self.engine_error = Some(e);
            event_loop.exit();
        }
    }

    fn new_events(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop, _cause: winit::event::StartCause) {
        if self.frame_clock.elapsed() >= self.frame_time {
            self.ctx.as_ref().map(|ctx| ctx.request_window_redraw());
        }
    }
}


impl<P: ApplicationProxy> Application<P> {
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
        let frame_opt = render_ctx.begin_frame()?;

        if let Some(frame) = frame_opt {
            let mut target = RenderTarget::new(&mut render_ctx, frame.view());
            proxy.on_render(ctx, &mut target);
            target.submit();
            let queue = render_ctx.queue().clone();
            frame.present(&queue);
        }
        
        Ok(())
    }
}