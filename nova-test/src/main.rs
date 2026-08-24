use nova_core::{EngineResult, app::{ApplicationBuilder, ApplicationContext, ApplicationProxy}, graphics::{color::Color, frame::Frame, render_pass::RenderPassDescriptor, sampler::{Sampler, SamplerMetadata}, shader::{Shader, ShaderMetadata}, texture::{Texture, TextureMetadata}}};

pub struct AppProxy;

impl ApplicationProxy for AppProxy {
    fn on_update(&mut self, _ctx: &mut ApplicationContext, _dt: std::time::Duration) {
    }
    
    fn on_render(&mut self, _ctx: &ApplicationContext, frame: &mut Frame) {
        frame.begin_render_pass(RenderPassDescriptor::default().with_color_clear(Color::BLUE));
    }
    
    fn on_init(&mut self, ctx: &mut ApplicationContext) -> EngineResult<()> {
        let _handle = ctx.assets_manager.load::<Shader>(ShaderMetadata::from_file("assets/shader.wgsl"))?;

        let sampler = ctx.assets_manager.load::<Sampler>(SamplerMetadata::default())?;
        let _handle = ctx.assets_manager.load::<Texture>(TextureMetadata::from_file("assets/happy-tree.png", sampler))?;

        Ok(())
    }
}

fn main() -> EngineResult<()> {
    simple_logger::init().unwrap();

    ApplicationBuilder::new(AppProxy)
    .build()
    .run()
}
