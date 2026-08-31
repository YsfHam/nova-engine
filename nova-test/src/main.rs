
use std::time::{Duration, Instant};

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
            material::{Material, MaterialMetadata},
            render_pass::RenderPassDescriptor,
            shader::ShaderStage,
            texture::{Texture, TextureMetadata, TextureSize},
            uniform::UniformValue,
        },
        math::{Vec2, vec2},
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

// ─── Stress test configuration ──────────────────────────────────────────────

/// Starting quad count before auto-ramp kicks in.
/// We already know 128K quads runs at ~36 FPS, so start there to save time.
const INITIAL_QUAD_COUNT: usize = 1_000_000;

/// If the rolling-average frame time stays below this, the ramp doubles the
/// quad count. 50.0 ms (~20 FPS) — only ramp up when we still have comfortable
/// headroom before the cliff.
const TARGET_FRAME_TIME_MS: f64 = 50.0;

/// If the average frame time exceeds this, the ramp stops — we've found the
/// cliff. 200.0 ms = 5 FPS: the point where the renderer truly dies.
const CLIFF_FRAME_TIME_MS: f64 = 200.0;

/// When we're above target but below the cliff, increase the quad count by
/// this many quads per stats interval (linear probing).
const RAMP_INCREMENT: usize = 32_000;

/// How many frames to accumulate before evaluating the ramp and printing
/// stats. At 60 FPS, 120 frames ≈ 2 seconds.
const STATS_INTERVAL_FRAMES: u64 = 120;

// ─── Procedural textures ────────────────────────────────────────────────────

/// Generates an RGBA8 checkerboard texture of `size`×`size` pixels with
/// `cells`×`cells` cells. `base` is the dark cell color, `light` is the light
/// cell color.
fn checker_texture(size: u32, cells: u32, base: [u8; 4], light: [u8; 4]) -> Vec<u8> {
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let cx = x * cells / size;
            let cy = y * cells / size;
            let is_light = (cx + cy) % 2 == 0;
            let c = if is_light { light } else { base };
            data.extend_from_slice(&c);
        }
    }
    data
}

// ─── Quad archetype ─────────────────────────────────────────────────────────

/// A quad "archetype" — determines material, color, UV, and whether it rotates.
/// Cycling through archetypes gives the stress test a mix of batch paths
/// (different materials → different draw batches) and visual variety.
#[derive(Clone, Copy)]
enum QuadArchetype {
    /// Default white texture + vertex color. Tests the flat-color path.
    FlatColor,
    /// Checker texture, white tint. Tests the pure-texture path.
    Textured,
    /// Checker texture + colored tint. Tests texture × color modulation.
    TexturedTinted,
    /// Rotated flat-color quad. Tests transform with rotation.
    RotatedFlat,
    /// Rotated textured quad.
    RotatedTextured,
    /// Rotated textured + tinted quad with a sub-rect UV (atlas simulation).
    RotatedSubUv,
}

impl QuadArchetype {
    const COUNT: usize = 6;

    fn from_index(i: usize) -> Self {
        match i % Self::COUNT {
            0 => Self::FlatColor,
            1 => Self::Textured,
            2 => Self::TexturedTinted,
            3 => Self::RotatedFlat,
            4 => Self::RotatedTextured,
            _ => Self::RotatedSubUv,
        }
    }

    fn rotates(self) -> bool {
        matches!(
            self,
            Self::RotatedFlat | Self::RotatedTextured | Self::RotatedSubUv
        )
    }
}

// ─── Application proxy ──────────────────────────────────────────────────────

pub struct AppProxy {
    // Materials — one per visual "kind" so the batcher produces multiple
    // draw batches (testing multi-batch / multi-draw-call performance).
    flat_material: Option<Handle<Material>>,
    checker_red_material: Option<Handle<Material>>,
    checker_green_material: Option<Handle<Material>>,

    // Quad cache: quads are deterministic (modulo total_time for rotation, but
    // we snapshot at cache-fill time). We build the cache lazily — when the
    // quad count grows we extend it. This isolates the batcher/render cost
    // from the per-quad construction cost.
    quad_cache: Vec<Quad>,

    // Stress-test state
    quad_count: usize,
    ramp_active: bool,
    frame_counter: u64,
    last_stats_frame: u64,
    app_start: Instant,

    // Three separate timing buckets, accumulated over STATS_INTERVAL_FRAMES.
    draw_ms: Vec<f64>,    // time in the draw_quad loop (all quads per frame)
    submit_ms: Vec<f64>,  // time in end_scene (batcher flush + submit)
    total_ms: Vec<f64>,   // begin_scene → end_scene (whole render process)

    // Animation
    total_time: f32,
}

impl AppProxy {
    pub fn new() -> Self {
        Self {
            flat_material: None,
            checker_red_material: None,
            checker_green_material: None,
            quad_cache: Vec::new(),
            quad_count: INITIAL_QUAD_COUNT,
            ramp_active: true,
            frame_counter: 0,
            last_stats_frame: 0,
            app_start: Instant::now(),
            draw_ms: Vec::with_capacity(STATS_INTERVAL_FRAMES as usize),
            submit_ms: Vec::with_capacity(STATS_INTERVAL_FRAMES as usize),
            total_ms: Vec::with_capacity(STATS_INTERVAL_FRAMES as usize),
            total_time: 0.0,
        }
    }

    /// Builds the quad for global index `i`, cycling through archetypes and
    /// arranging quads in a scrolling grid.
    fn make_quad(&self, i: usize, screen: Vec2) -> Quad {
        let archetype = QuadArchetype::from_index(i);
        let (material, color, uv) = match archetype {
            QuadArchetype::FlatColor | QuadArchetype::RotatedFlat => {
                // Cycle through named colors for visual variety.
                let color = match i % 4 {
                    0 => Color::YELLOW,
                    1 => Color::CYAN,
                    2 => Color::MAGENTA,
                    _ => Color::RED,
                };
                (self.flat_material.unwrap(), color, full_uv())
            }
            QuadArchetype::Textured | QuadArchetype::RotatedTextured => {
                (self.checker_red_material.unwrap(), Color::WHITE, full_uv())
            }
            QuadArchetype::TexturedTinted => {
                (self.checker_red_material.unwrap(), Color::GREEN, full_uv())
            }
            QuadArchetype::RotatedSubUv => {
                // Sub-rect UV: use only the top-left quadrant of the texture.
                (
                    self.checker_green_material.unwrap(),
                    Color::YELLOW,
                    RectF32 {
                        top: 0.0,
                        left: 0.0,
                        bottom: 0.5,
                        right: 0.5,
                    },
                )
            }
        };

        // Grid layout: arrange quads in rows across the screen.
        // Quad size is small so we can fit many on screen.
        let quad_size = 16.0_f32;
        let cols = (screen.x / quad_size).ceil() as usize;
        let col = i % cols;
        let row = i / cols;
        let x = col as f32 * quad_size + quad_size * 0.5;
        let y = row as f32 * quad_size + quad_size * 0.5;

        // Animate rotation for rotated archetypes.
        let angle = if archetype.rotates() {
            self.total_time * 2.0 + i as f32 * 0.1
        } else {
            0.0
        };

        // Wrap y so quads that overflow the screen wrap back to the top.
        let y_wrapped = y % screen.y;

        // Spread across a few z-layers to test the BTreeMap layer grouping.
        let z_index = (i % 3) as u32;

        Quad::new(material)
            .with_position(vec2(x, y_wrapped))
            .with_scale(vec2(quad_size, quad_size))
            .with_color(color)
            .with_angle(angle)
            .with_z_index(z_index)
            .with_uv(uv)
    }

    /// Ensures the quad cache has at least `quad_count` entries, building
    /// new quads for any indices beyond the current cache size.
    fn refill_cache(&mut self, screen: Vec2) {
        if self.quad_cache.len() >= self.quad_count {
            return;
        }
        let start = self.quad_cache.len();
        for i in start..self.quad_count {
            let quad = self.make_quad(i, screen);
            self.quad_cache.push(quad);
        }
    }

    /// Refreshes the animated fields (rotation angle) on all cached quads.
    /// Static fields (material, color, UV, position, scale, z_index) stay
    /// as built by `make_quad` — only the angle depends on `total_time`.
    fn update_cache(&mut self) {
        for (i, quad) in self.quad_cache.iter_mut().enumerate() {
            if QuadArchetype::from_index(i).rotates() {
                quad.angle = self.total_time * 2.0 + i as f32 * 0.1;
            }
        }
    }

    fn evaluate_ramp(&mut self) {
        if !self.ramp_active || self.total_ms.is_empty() {
            return;
        }

        let avg_ms = self.total_ms.iter().sum::<f64>() / self.total_ms.len() as f64;

        if avg_ms > CLIFF_FRAME_TIME_MS {
            // We've hit the cliff — stop ramping and report.
            self.ramp_active = false;
            println!(
                "\n=== CLIFF DETECTED ===\n\
                 Quad count: {}\n\
                 Avg total frame time: {:.2} ms ({:.0} FPS)\n\
                 Stopping auto-ramp.\n",
                self.quad_count,
                avg_ms,
                1000.0 / avg_ms
            );
            std::process::exit(0);
        } else if avg_ms < TARGET_FRAME_TIME_MS {
            // Plenty of headroom — double the quad count.
            let old = self.quad_count;
            self.quad_count = self.quad_count.saturating_mul(2);
            if self.quad_count != old {
                println!(
                    "[ramp] {:.2} ms avg — doubling {} → {} quads",
                    avg_ms, old, self.quad_count
                );
            }
        } else {
            // Above target but below cliff — linear probe by RAMP_INCREMENT.
            let old = self.quad_count;
            self.quad_count = self.quad_count.saturating_add(RAMP_INCREMENT);
            if self.quad_count != old {
                println!(
                    "[ramp] {:.2} ms avg — increasing {} → {} quads (+{})",
                    avg_ms, old, self.quad_count, RAMP_INCREMENT
                );
            }
        }
    }

    fn print_stats(&self) {
        if self.total_ms.is_empty() {
            return;
        }
        let n = self.total_ms.len() as f64;
        let avg_total = self.total_ms.iter().sum::<f64>() / n;
        let avg_draw = self.draw_ms.iter().sum::<f64>() / n;
        let avg_submit = self.submit_ms.iter().sum::<f64>() / n;
        let max_total = self.total_ms.iter().cloned().fold(0.0_f64, f64::max);
        let fps = 1000.0 / avg_total;
        let elapsed = self.app_start.elapsed().as_secs();

        println!(
            "[{:>4}s] quads: {:>8} | total: {:>7.2} ms (~{:>5.0} FPS) | draw: {:>7.2} ms | end_scene: {:>7.2} ms | max: {:>7.2} | ramp: {}",
            elapsed,
            self.quad_count,
            avg_total,
            fps,
            avg_draw,
            avg_submit,
            max_total,
            if self.ramp_active { "ON" } else { "OFF" },
        );
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

impl ApplicationProxy for AppProxy {
    fn on_init(&mut self, ctx: &mut ApplicationContext) -> EngineResult<()> {
        // Grab the default white-texture material and its template.
        // The Nova2DPlugin already loaded the template + a default material.
        let default_material =
            ctx.default_assets.expect(Nova2dDefaults::WhiteTextureMaterial);
        let template =
            ctx.default_assets.expect(Nova2dDefaults::TexturedQuadMaterialTemplate);

        // The flat-color material is the plugin's default (white texture +
        // vertex color modulation). Reuse it directly.
        self.flat_material = Some(default_material);

        // Create two checkerboard textures for the textured/tinted variants.
        let default_sampler = ctx
            .default_assets
            .expect(nova::core::assets::defaults::CoreDefaultAssets::DefaultSampler);

        let red_checker_data =
            checker_texture(64, 8, [0x30, 0x10, 0x10, 0xFF], [0xE0, 0x40, 0x40, 0xFF]);
        let red_checker = ctx.assets_manager.load::<Texture>(TextureMetadata::from_raw(
            "red_checker",
            red_checker_data,
            TextureSize::new_texture2d(64, 64),
            default_sampler,
        ))?;

        let green_checker_data = checker_texture(
            64,
            8,
            [0x10, 0x30, 0x10, 0xFF],
            [0x40, 0xE0, 0x40, 0xFF],
        );
        let green_checker = ctx.assets_manager.load::<Texture>(TextureMetadata::from_raw(
            "green_checker",
            green_checker_data,
            TextureSize::new_texture2d(64, 64),
            default_sampler,
        ))?;

        // Materials using the shared template + different textures.
        // All three materials share the same pipeline (same template) —
        // only bind groups differ, so pipeline compilation is deduplicated.
        self.checker_red_material = Some(ctx.assets_manager.load::<Material>(
            MaterialMetadata::new(template).with_texture(0, red_checker),
        )?);
        self.checker_green_material = Some(ctx.assets_manager.load::<Material>(
            MaterialMetadata::new(template).with_texture(0, green_checker),
        )?);

        Ok(())
    }

    fn on_update(&mut self, _ctx: &mut ApplicationContext, dt: Duration) {
        self.total_time += dt.as_secs_f32();
    }

    fn on_render(&mut self, ctx: &ApplicationContext, frame: &mut Frame) {
        let (width, height) = ctx.window_api.size();
        let screen = vec2(width as f32, height as f32);

        let camera = Camera2D::with_size(screen);

        // Lazily grow the quad cache to match the current quad count,
        // then refresh animated fields (rotation) on all cached quads.
        self.refill_cache(screen);
        self.update_cache();

        let mut target = frame.render_target(&ctx.render_ctx);

        let commander = target.commander(
            EnvironmentDescriptor::new().add_uniform(EnvironmentUniform {
                binding_slot: 0,
                visibilty: ShaderStage::Vertex,
                uniform: UniformValue::Mat4(camera.projection()),
            }),
        );

        // ── Three-way timing: draw loop / end_scene / total ──────────────
        let total_start = Instant::now();

        let mut renderer = Render2D::begin_scene(commander);

        let draw_start = Instant::now();
        let count = self.quad_count;
        let cache = &self.quad_cache;
        for i in 0..count {
            renderer.draw_quad(cache[i]);
        }
        let draw_elapsed = draw_start.elapsed();

        let submit_start = Instant::now();
        renderer.end_scene(RenderPassDescriptor::new(), &ctx.assets_manager);
        let submit_elapsed = submit_start.elapsed();

        let total_elapsed = total_start.elapsed();

        // ── Stats tracking ────────────────────────────────────────────────
        self.draw_ms.push(draw_elapsed.as_secs_f64() * 1000.0);
        self.submit_ms.push(submit_elapsed.as_secs_f64() * 1000.0);
        self.total_ms.push(total_elapsed.as_secs_f64() * 1000.0);
        self.frame_counter += 1;

        if self.frame_counter - self.last_stats_frame >= STATS_INTERVAL_FRAMES {
            self.print_stats();
            self.evaluate_ramp();
            self.draw_ms.clear();
            self.submit_ms.clear();
            self.total_ms.clear();
            self.last_stats_frame = self.frame_counter;
        }
    }
}

fn main() -> EngineResult<()> {
    //simple_logger::init_with_env().unwrap();

    println!("=== Nova Engine — 2D Stress Test ===");
    println!(
        "Starting with {} quads, auto-ramping to find the FPS cliff.",
        INITIAL_QUAD_COUNT
    );
    println!(
        "Target: stay under {:.1} ms/frame. Cliff: {:.1} ms/frame.",
        TARGET_FRAME_TIME_MS, CLIFF_FRAME_TIME_MS
    );
    println!();

    ApplicationBuilder::new(AppProxy::new())
        .alter_window_attributes(|win_attr| win_attr.with_inner_size(LogicalSize::new(800, 600)))
        .with_plugins(DefaultPlugins)
        .build()
        .run()
}
