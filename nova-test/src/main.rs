use nova_core::{EngineResult, app::{ApplicationBuilder, ApplicationContext, ApplicationProxy}, graphics::{color::Color, render_target::RenderTarget, render_pass::RenderPassDescriptor}};

pub struct AppProxy;

impl ApplicationProxy for AppProxy {
    fn on_update(&mut self, _ctx: &mut ApplicationContext, _dt: std::time::Duration) {
    }
    
    fn on_render(&mut self, _ctx: &ApplicationContext, target: &mut RenderTarget<'_>) {
        target.begin_render_pass(RenderPassDescriptor::default().with_color_clear(Color::BLUE));
    }
    
    fn on_init(&mut self, _ctx: &mut ApplicationContext) -> EngineResult<()> {
        Ok(())
    }
}

fn main() -> EngineResult<()> {
    simple_logger::init().unwrap();

    ApplicationBuilder::new(AppProxy)
    .build()
    .run()
}
