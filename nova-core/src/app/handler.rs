
use std::time::{Duration, Instant};

use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::{ActiveEventLoop, ControlFlow}};

use crate::{EngineResult, app::{Application, ApplicationContext, ApplicationProxy}};

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
                ctx.render_ctx.get().resize_surface(size.width, size.height);
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

    fn on_render(proxy: &mut P, ctx: &ApplicationContext) -> EngineResult<()> {
        // begin_frame borrows the RefCell mutably, returns a Frame that owns
        // the surface texture + view (no borrow of the RefCell). Guard drops.
        let frame_opt = ctx.render_ctx.get_mut().begin_frame()?;

        if let Some(mut frame) = frame_opt {
            // The proxy records commands: it creates a RenderTarget (holding
            // a RefMut guard) from the frame, records via its commander, and
            // submits (consuming the target + dropping the guard).
            proxy.on_render(ctx, &mut frame);
            // The RefMut guard is dropped by now; present uses a fresh
            // immutable borrow to access the queue.

            ctx.render_ctx.get_mut().submit_commands();
            frame.present(&ctx.render_ctx);
        }
        
        Ok(())
    }
}