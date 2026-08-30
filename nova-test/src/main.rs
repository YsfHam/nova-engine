
use std::f32::consts::PI;

use nova::{DefaultPlugins, core::{
    EngineResult, app::{ApplicationBuilder, ApplicationContext, ApplicationProxy}, graphics::{
        color::Color, environment::{EnvironmentDescriptor, EnvironmentUniform}, frame::Frame, render_pass::RenderPassDescriptor, shader::ShaderStage, uniform::UniformValue,
    }, math::{Vec2, vec2}, window::LogicalSize,
}, nova2d::{camera::Camera2D, defaults::Nova2dDefaults, quad::Quad, render2d::Render2D}};

pub struct AppProxy {
}

impl AppProxy {
    pub fn new() -> Self {
        Self {}
    }
}

impl ApplicationProxy for AppProxy {
    fn on_update(&mut self, _ctx: &mut ApplicationContext, _dt: std::time::Duration) {
    }

    fn on_render(&mut self, ctx: &ApplicationContext, frame: &mut Frame) {
        let material_handle = ctx.default_assets.expect(Nova2dDefaults::WhiteTextureMaterial);

        let (width, height) = ctx.window_api.size();

        let camera = Camera2D::with_position_and_size(
            Vec2::ZERO,
            vec2(width as f32, height as f32)
        );


        let mut target = frame.render_target(&ctx.render_ctx);

        let commander = target.commander(
            EnvironmentDescriptor::new()
            .add_uniform(EnvironmentUniform {
                binding_slot: 0,
                visibilty: ShaderStage::Vertex,
                uniform: UniformValue::Mat4(camera.projection()),
            })
        );

        let mut renderer = Render2D::begin_scene(commander);
        renderer.draw_quad(
            Quad::new(material_handle)
            .with_position(vec2(100.0, 90.0))
            .with_scale(vec2(100.0, 100.0))
            .with_color(Color::YELLOW)
            .with_angle(PI / 4.0)
        );

        renderer.draw_quad(
            Quad::new(material_handle)
            .with_position(vec2(200.0, 90.0))
            .with_scale(vec2(100.0, 100.0))
            .with_color(Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 0.5,
            })
            .with_angle(PI / 4.0)
        );

        renderer.end_scene(
            RenderPassDescriptor::new(),
            &ctx.assets_manager
        );

    }

    fn on_init(&mut self, _ctx: &mut ApplicationContext) -> EngineResult<()> {
        Ok(())
    }
}

fn main() -> EngineResult<()> {
    simple_logger::init_with_env().unwrap();

    ApplicationBuilder::new(AppProxy::new())
        .alter_window_attributes(|win_attr| 
            win_attr.with_inner_size(LogicalSize::new(800, 600))
        )
        .with_plugins(DefaultPlugins)
        .build()
        .run()
}
