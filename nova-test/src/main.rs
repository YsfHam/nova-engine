use nova_core::{EngineResult, app::{ApplicationBuilder, ApplicationContext, ApplicationProxy}, graphics::{color::Color, frame::Frame, render_pass::RenderPassDescriptor}};

pub struct AppProxy;

impl ApplicationProxy for AppProxy {
    fn on_update(&mut self, _ctx: &mut ApplicationContext, _dt: std::time::Duration) {
    }
    
    fn on_render(&mut self, _ctx: &ApplicationContext, frame: &mut Frame) {
        frame.begin_render_pass(RenderPassDescriptor::default().with_color_clear(Color::BLUE));
    }
}

fn main() -> EngineResult<()> {
    simple_logger::init().unwrap();

    ApplicationBuilder::new(AppProxy)
    .build()
    .run()
}
