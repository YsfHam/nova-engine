
use std::time::Duration;

use nova::{
    DefaultPlugins, core::{
        EngineResult, app::{ApplicationBuilder, ApplicationContext, ApplicationProxy}, assets::{defaults::CoreDefaultAssets, handle::Handle}, graphics::{
            color::Color, environment::{EnvironmentDescriptor, EnvironmentUniform}, frame::Frame, material::Material, render_pass::RenderPassDescriptor, sampler::{FilterMode, Sampler, SamplerMetadata}, shader::ShaderStage, texture::TextureMetadata, uniform::UniformValue,
        }, math::vec2, time::Clock, window::LogicalSize,
    }, nova2d::{
        camera::Camera2D, defaults::{Nova2dDefaults, create_material_with_texture_meta}, quad::Quad, render2d::Render2D, sprite::SpriteAtlas,
    },
};

mod stress;

// ─── Overlap demo proxy ─────────────────────────────────────────────────────

/// A minimal application that draws two quads that overlap partially.
///
/// Quad A (red) is drawn at `z_index = 0` and quad B (blue) at `z_index = 1`,
/// so the blue quad appears on top in the overlapping region. Both quads are
/// semi-transparent (alpha 0.6) to make the overlap region clearly visible.
pub struct App {
    material: Option<Handle<Material>>,
    tree_material: Option<Handle<Material>>,
    sprite_atlas: Option<SpriteAtlas>,
    sprite_index: u32,
    total_time: Clock,
    animation_time: Clock,
}

impl App {
    pub fn new() -> Self {
        Self {
            material: None,
            tree_material: None,
            sprite_atlas: None,
            sprite_index: 0,
            total_time: Clock::new(),
            animation_time: Clock::new(),
        }
    }
}

impl ApplicationProxy for App {
    fn on_init(&mut self, ctx: &mut ApplicationContext) -> EngineResult<()> {
        // Reuse the plugin's default white-texture material. Vertex color
        // (including alpha) modulates the white texture, giving us flat color
        // quads with per-quad alpha.
        self.material = Some(ctx.default_assets.expect(Nova2dDefaults::WhiteTextureMaterial));

        self.tree_material = Some(
            create_material_with_texture_meta(
                ctx, 
                TextureMetadata::from_file(
                    "./nova-test/assets/tree.png", 
                    ctx.default_assets.expect(CoreDefaultAssets::DefaultSampler)
                )
            )?
        );

        let atlas_mat = 
            create_material_with_texture_meta(
                ctx,
                TextureMetadata::from_file(
                    "C:\\dev\\nova-engine\\nova-test\\assets\\sample(idle&walk)\\walk\\sprite sheets\\walk.png",
                    ctx.default_assets.expect(Nova2dDefaults::PixelatedSampler)
                )
            )?;
        let atlas_size = vec2(180.0, 348.0);
        let cell_size = vec2(45.0, 58.0);
        self.sprite_atlas = Some(SpriteAtlas::new(
            atlas_mat, 
            atlas_size, 
            cell_size,
        ));

        Ok(())
    }

    fn on_update(&mut self, _ctx: &mut ApplicationContext, _dt: Duration) {
        if self.animation_time.elapsed().as_millis() > (1000/24) {
            self.sprite_index += 1;
            self.animation_time.restart();
        }
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
        let total_time = self.total_time.elapsed().as_secs_f32();
        let sway = (total_time * 1.5).sin() * 40.0;
        let sway_y = (total_time * 1.5).cos() * 40.0;

        let cx = screen.x * 0.5;
        let cy = screen.y * 0.5;


        // Quad A — red, bottom-left of center, z = 0.
        let quad_a = Quad::new(self.material.unwrap())
            .with_position(vec2(cx - offset + sway, cy + sway_y))
            .with_scale(vec2(size, size))
            .with_color(Color { r: 1.0, g: 0.2, b: 0.2, a: 0.6 })
            .with_z_index(0);

        // Quad B — blue, top-right of center, z = 1 (drawn on top).
        let quad_b = Quad::new(self.material.unwrap())
            .with_position(vec2(cx + offset + sway, cy + sway_y))
            .with_scale(vec2(size, size))
            .with_color(Color { r: 0.2, g: 0.3, b: 1.0, a: 0.6 })
            .with_z_index(1);

        let quad_tree_tex = Quad::new(self.tree_material.unwrap())
            .with_position(vec2((cx + offset + sway) * 0.5, cy - offset + sway_y))
            .with_scale(vec2(size, size))
            .with_angle(sway * sway_y)
            .with_angle(total_time * 0.5);

        let sprite_atlas = self.sprite_atlas.as_ref().unwrap();
        //self.sprite_index = 5;
        let sprite = match sprite_atlas.sprite(self.sprite_index) {
            Some(sprite) => sprite,
            None => {
                //std::process::exit(0);
                self.sprite_index = 0;
                sprite_atlas.sprite(0).unwrap()
            }
        };

        let character = sprite
            .with_position((100.0, 200.0).into())
            .with_scale(vec2(45.0, 58.0) * 3.0) ;

        let mut target = frame.render_target(&ctx.render_ctx);

        let commander = target.commander(
            EnvironmentDescriptor::new().add_uniform(EnvironmentUniform {
                binding_slot: 0,
                visibilty: ShaderStage::Vertex,
                uniform: UniformValue::Mat4(camera.projection()),
            }),
        );

        let mut renderer = Render2D::begin_scene(commander);
        // renderer.draw(quad_a);
        // renderer.draw(quad_b);
        // renderer.draw(quad_tree_tex);
        renderer.draw(character);
        renderer.end_scene(RenderPassDescriptor::new(), &ctx.assets_manager);
    }
}

fn main() -> EngineResult<()> {
    //simple_logger::init_with_env().unwrap();

    println!("=== Nova Engine — Overlap Demo ===");
    println!("Two semi-transparent quads that partially overlap.");

    ApplicationBuilder::new(App::new())
        .alter_window_attributes(|win_attr| win_attr.with_inner_size(LogicalSize::new(800, 600)))
        .with_plugins(DefaultPlugins)
        .build()
        .run()
}
