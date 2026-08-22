use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::ControlFlow};

use crate::app::{Application, ApplicationProxy};

impl<P: ApplicationProxy> ApplicationHandler for Application<P> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
       self.init(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {

        let ctx = self.ctx.as_ref().unwrap();
        let proxy = &mut self.proxy;

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::RedrawRequested => {
                proxy.on_update(ctx);

                if self.control_flow == ControlFlow::Poll {
                    ctx.request_window_redraw();
                }
            }

            _ => ()
        }
    }
}