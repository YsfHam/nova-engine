
use std::time::Duration;

use nova::{
    DefaultPlugins, core::{
        EngineResult, app::{ApplicationBuilder, ApplicationContext, ApplicationProxy}, assets::handle::Handle, graphics::{
            color::Color,
            frame::Frame,
            material::{BindGroup, BindGroupEntry},
            render_pass::RenderPassDescriptor,
            sampler::FilterMode,
            shader::ShaderStage,
            texture::{Texture, TextureConfig},
            uniform::UniformValue,
        }, math::{Angle, vec2}, time::Clock, window::LogicalSize,
    }, nova2d::{
        camera::Camera2D,
        defaults::Nova2dDefaults,
        materials::{ColorMaterial, SpriteMaterial},
        render2d::Render2D,
        sprite::{Sprite, SpriteAtlas},
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
    color_material: Option<Handle<ColorMaterial>>,
    tree_material: Option<Handle<SpriteMaterial>>,
    sprite_atlas: Option<SpriteAtlas>,
    sprite_index: u32,
    total_time: Clock,
    animation_time: Clock,
}

impl App {
    pub fn new() -> Self {
        Self {
            color_material: None,
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
        // Default color material — vertex color (including alpha) modulates
        // the uniform color, giving us flat color sprites with per-sprite alpha.
        self.color_material = Some(
            ctx.default_assets
                .expect::<ColorMaterial>(Nova2dDefaults::DefaultColorMaterial),
        );

        // Tree texture: load a CPU Texture asset, then wrap it in a SpriteMaterial.
        let pixelated = TextureConfig {
            sampler_config: nova::core::graphics::sampler::SamplerConfig {
                mag_filter: FilterMode::Nearest,
                ..Default::default()
            },
            ..Default::default()
        };
        let tree_texture =
            Texture::from_file("./nova-test/assets/tree.png", pixelated.clone())
                .map_err(|e| nova::core::errors::EngineError::UserError(e.to_string()))?;
        let tree_texture_handle = ctx.assets_manager.insert_asset(tree_texture);
        let tree_material = SpriteMaterial::new(tree_texture_handle);
        self.tree_material = Some(ctx.assets_manager.insert_asset(tree_material));

        // Walk-cycle atlas: load the sprite sheet, wrap in SpriteMaterial, build atlas.
        let walk_texture = Texture::from_file(
            "C:\\dev\\nova-engine\\nova-test\\assets\\sample(idle&walk)\\walk\\sprite sheets\\walk.png",
            pixelated,
        )
        .map_err(|e| nova::core::errors::EngineError::UserError(e.to_string()))?;
        let walk_texture_handle = ctx.assets_manager.insert_asset(walk_texture);
        let walk_material = SpriteMaterial::new(walk_texture_handle);
        let walk_material_handle = ctx.assets_manager.insert_asset(walk_material);

        let atlas_size = vec2(180.0, 348.0);
        let cell_size = vec2(45.0, 58.0);
        self.sprite_atlas = Some(SpriteAtlas::new(
            walk_material_handle.into_generic(),
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

        // Top-left camera: world (0, 0) = top-left of screen.
        let cx = screen.x * 0.5;
        let cy = screen.y * 0.5;

        // Sprite size — large enough to overlap comfortably.
        let size: f32 = 300.0;

        // Offset from center for each sprite. With a 300px sprite and a 120px
        // offset, the overlap region is 300 - 120 = 180px wide.
        let offset: f32 = 120.0;

        // Gentle oscillation so the overlap changes over time.
        let total_time = self.total_time.elapsed().as_secs_f32();
        let sway = (total_time * 1.5).sin() * 40.0;
        let sway_y = (total_time * 1.5).cos() * 40.0;

        let mut camera = Camera2D::with_size(screen);
        camera.rotation = Angle::Degrees(45.0);
        camera.zoom = 2.0;

        // Sprite A — red, bottom-left of center, z = 0.
        let _sprite_a = Sprite::new(self.color_material.unwrap().into_generic())
            .with_position(vec2(cx - offset + sway, cy + sway_y))
            .with_scale(vec2(size, size))
            .with_color(Color { r: 1.0, g: 0.2, b: 0.2, a: 0.6 })
            .with_z_index(0);

        // Sprite B — blue, top-right of center, z = 1 (drawn on top).
        let _sprite_b = Sprite::new(self.color_material.unwrap().into_generic())
            .with_position(vec2(cx + offset + sway, cy + sway_y))
            .with_scale(vec2(size, size))
            .with_color(Color { r: 0.2, g: 0.3, b: 1.0, a: 0.6 })
            .with_z_index(1);

        let _sprite_tree = Sprite::new(self.tree_material.unwrap().into_generic())
            .with_position(vec2((cx + offset + sway) * 0.5, cy - offset + sway_y))
            .with_scale(vec2(size, size))
            .with_angle((total_time * 0.5).into());

        let sprite_atlas = self.sprite_atlas.as_ref().unwrap();
        let sprite = match sprite_atlas.sprite(self.sprite_index) {
            Some(sprite) => sprite,
            None => {
                self.sprite_index = 0;
                sprite_atlas.sprite(0).unwrap()
            }
        };

        let character = sprite
            .with_position((cx, cy).into())
            .with_scale(vec2(45.0, 58.0) * 3.0);

        let mut target = frame.render_target(&ctx.render_ctx);

        let environment = BindGroup::new().with_entry(BindGroupEntry::Uniform {
            binding_slot: 0,
            visibility: ShaderStage::Vertex,
            value: UniformValue::Mat4(camera.projection()),
        });
        let commander = target.commander(environment);

        let mut renderer = Render2D::begin_scene(commander);
        renderer.draw(_sprite_a);
        renderer.draw(_sprite_b);
        renderer.draw(_sprite_tree);
        renderer.draw(character);
        renderer.end_scene(RenderPassDescriptor::new(), &ctx.assets_manager);
    }
}

fn main() -> EngineResult<()> {
    //simple_logger::init_with_env().unwrap();

    println!("=== Nova Engine — Overlap Demo ===");
    println!("Two semi-transparent sprites that partially overlap.");

    ApplicationBuilder::new(App::new())
        .alter_window_attributes(|win_attr| win_attr.with_inner_size(LogicalSize::new(800, 600)))
        .with_plugins(DefaultPlugins)
        .build()
        .run()
}
