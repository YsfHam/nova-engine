
use std::time::Duration;

use nova::{
    DefaultPlugins,
    core::{
        EngineResult,
        app::{ApplicationBuilder, ApplicationContext, ApplicationProxy},
        assets::handle::Handle,
        graphics::{
            color::Color,
            environment::{EnvironmentDescriptor, EnvironmentUniform},
            frame::Frame,
            material::Material,
            render_pass::RenderPassDescriptor,
            shader::ShaderStage,
            uniform::UniformValue,
        },
        math::vec2,
        window::LogicalSize,
    },
    nova2d::{
        camera::Camera2D,
        defaults::Nova2dDefaults,
        quad::Quad,
        render2d::Render2D,
        utils::RectF32,
    },
};

mod stress;

// ─── Overlap demo proxy ─────────────────────────────────────────────────────

/// A minimal application that draws two quads that overlap partially.
///
/// Quad A (red) is drawn at `z_index = 0` and quad B (blue) at `z_index = 1`,
/// so the blue quad appears on top in the overlapping region. Both quads are
/// semi-transparent (alpha 0.6) to make the overlap region clearly visible.
pub struct OverlapProxy {
    material: Option<Handle<Material>>,
    total_time: f32,
}

impl OverlapProxy {
    pub fn new() -> Self {
        Self {
            material: None,
            total_time: 0.0,
        }
    }
}

fn full_uv() -> RectF32 {
    RectF32 {
        top: 0.0,
        left: 0.0,
        bottom: 1.0,
        right: 1.0,
    }
}

impl ApplicationProxy for OverlapProxy {
    fn on_init(&mut self, ctx: &mut ApplicationContext) -> EngineResult<()> {
        // Reuse the plugin's default white-texture material. Vertex color
        // (including alpha) modulates the white texture, giving us flat color
        // quads with per-quad alpha.
        self.material = Some(ctx.default_assets.expect(Nova2dDefaults::WhiteTextureMaterial));
        Ok(())
    }

    fn on_update(&mut self, _ctx: &mut ApplicationContext, dt: Duration) {
        self.total_time += dt.as_secs_f32();
    }

    fn on_render(&mut self, ctx: &ApplicationContext, frame: &mut Frame) {
        let (width, height) = ctx.window_api.size();
        let screen = vec2(width as f32, height as f32);

        // Centered camera: world (0, 0) = screen center.
        let camera = Camera2D::with_size(screen);

        // Quad size — large enough to overlap comfortably.
        let size: f32 = 300.0;

        // Offset from center for each quad. With a 300px quad and a 120px
        // offset, the overlap region is 300 - 120 = 180px wide.
        let offset: f32 = 120.0;

        // Gentle oscillation so the overlap changes over time.
        let sway = (self.total_time * 1.5).sin() * 40.0;
        let sway_y = (self.total_time * 1.5).cos() * 40.0;

        let cx = screen.x * 0.5;
        let cy = screen.y * 0.5;

        // Quad A — red, bottom-left of center, z = 0.
        let quad_a = Quad::new(self.material.unwrap())
            .with_position(vec2(cx - offset + sway, cy + sway_y))
            .with_scale(vec2(size, size))
            .with_color(Color { r: 1.0, g: 0.2, b: 0.2, a: 0.6 })
            .with_z_index(0)
            .with_uv(full_uv());

        // Quad B — blue, top-right of center, z = 1 (drawn on top).
        let quad_b = Quad::new(self.material.unwrap())
            .with_position(vec2(cx + offset + sway, cy + sway_y))
            .with_scale(vec2(size, size))
            .with_color(Color { r: 0.2, g: 0.3, b: 1.0, a: 0.6 })
            .with_z_index(1)
            .with_uv(full_uv());

        let mut target = frame.render_target(&ctx.render_ctx);

        let commander = target.commander(
            EnvironmentDescriptor::new().add_uniform(EnvironmentUniform {
                binding_slot: 0,
                visibilty: ShaderStage::Vertex,
                uniform: UniformValue::Mat4(camera.projection()),
            }),
        );

        let mut renderer = Render2D::begin_scene(commander);
        renderer.draw(quad_a);
        renderer.draw(quad_b);
        renderer.end_scene(RenderPassDescriptor::new(), &ctx.assets_manager);
    }
}

fn main() -> EngineResult<()> {
    //simple_logger::init_with_env().unwrap();

    println!("=== Nova Engine — Overlap Demo ===");
    println!("Two semi-transparent quads that partially overlap.");

    ApplicationBuilder::new(OverlapProxy::new())
        .alter_window_attributes(|win_attr| win_attr.with_inner_size(LogicalSize::new(800, 600)))
        .with_plugins(DefaultPlugins)
        .build()
        .run()
}
