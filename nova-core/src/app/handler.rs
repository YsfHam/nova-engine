
use winit::application::ApplicationHandler;

use crate::app::{Application, ApplicationProxy};

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