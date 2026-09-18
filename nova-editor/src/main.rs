use std::time::Duration;

use nova::{
    DefaultPlugins, core::{
        EngineResult, app::{ApplicationBuilder, ApplicationContext, ApplicationProxy}, assets::{AssetState, handle::StrongHandle}, graphics::{
            color::Color, frame::Frame, material::{BindGroup, BindGroupEntry}, render_pass::RenderPassDescriptor, render_target::TextureRenderTarget, sampler::{FilterMode, SamplerConfig}, shader::ShaderStage, texture::{Texture, TextureConfig, TextureFormat, TextureSize}, uniform::UniformValue,
        }, math::{Angle, vec2}, time::Clock,
    }, egui::{self, EguiTextureHandle}, nova2d::{
        camera::Camera2D,
        defaults::Nova2dDefaults,
        materials::SpriteMaterial,
        render2d::Render2D,
        shape::{CircleShape, ShapeInstance},
        sprite::Sprite,
    },
};

use egui_dock::{DockArea, DockState, Style, TabViewer};
use egui::{Id, Ui, WidgetText};

// ─── Editor tab system ──────────────────────────────────────────────────────

type Tab = String;

struct EditorTabViewer {
    scene_texture_id: Option<egui::TextureId>,
}

impl TabViewer for EditorTabViewer {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Self::Tab) -> Id {
        Id::new(tab)
    }

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        tab.as_str().into()
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match tab.as_str() {
            "Viewport" => {
                if let Some(tex_id) = self.scene_texture_id {
                    let size = ui.available_size();
                    ui.image(egui::load::SizedTexture::new(tex_id, size));
                } else {
                    ui.label("Scene texture not initialized.");
                }
            }
            "Inspector" => {
                ui.label("Inspector — properties will appear here.");
                ui.separator();
                ui.label("(Select an entity to inspect)");
            }
            _ => {
                ui.label(format!("Content of {tab}"));
            }
        }
    }
}

// ─── Editor app ─────────────────────────────────────────────────────────────

struct EditorApp {
    /// Off-screen render target — the 2D scene renders into this texture.
    scene_target: Option<TextureRenderTarget>,
    /// Egui texture handle for displaying the scene target in the viewport panel.
    scene_texture: Option<EguiTextureHandle>,
    /// Dock state for the editor layout.
    dock_state: DockState<Tab>,
    /// Sprite material for rendering rectangles.
    sprite_material: Option<nova::core::assets::handle::Handle<SpriteMaterial>>,
    /// Circle material for rendering circles.
    circle_material: Option<nova::core::assets::handle::Handle<nova::nova2d::materials::CircleMaterial>>,
    /// Elapsed time for animations.
    total_time: Clock,

    // ─── Async loading + off-loading demo ───────────────────────────────
    /// Strong handle to an async-loaded texture. While `Some`, the asset
    /// stays alive. When dropped (set to `None`), `cleanup_offloaded` will
    /// recycle the slot next frame.
    async_texture: Option<StrongHandle<Texture>>,
    /// Weak handle derived from the strong handle — used to poll state
    /// and look up the asset without keeping it alive.
    async_texture_weak: Option<nova::core::assets::handle::Handle<Texture>>,
    /// Number of frames since the load was dispatched (for reporting).
    load_frames: u32,
    /// Cumulative off-load count reported by `cleanup_offloaded`.
    offloaded_count: usize,
}

impl EditorApp {
    fn new() -> Self {
        let tabs = vec!["Viewport".to_string(), "Inspector".to_string()];
        Self {
            scene_target: None,
            scene_texture: None,
            dock_state: DockState::new(tabs),
            sprite_material: None,
            circle_material: None,
            total_time: Clock::new(),
            async_texture: None,
            async_texture_weak: None,
            load_frames: 0,
            offloaded_count: 0,
        }
    }

    /// Renders an animated 2D scene into the off-screen texture.
    fn render_scene(&self, ctx: &ApplicationContext) {
        let target = self.scene_target.as_ref().unwrap();
        let mut render_target = target.as_render_target(&ctx.render_ctx);

        let camera = Camera2D::with_size(vec2(800.0, 600.0));

        let environment = BindGroup::new().with_entry(BindGroupEntry::Uniform {
            binding_slot: 0,
            visibility: ShaderStage::Vertex,
            value: UniformValue::Mat4(camera.projection()),
        });
        let commander = render_target.commander(environment);

        let mut renderer = Render2D::begin_scene(commander);

        let t = self.total_time.elapsed().as_secs_f32();

        // Animated rectangles — orbiting around the center with pulsing scale.
        let cx = 400.0_f32;
        let cy = 300.0_f32;

        if let Some(mat) = self.sprite_material {
            // Red rect — orbits clockwise, pulses size.
            let angle_r = t * 1.2;
            let orbit_r = 180.0;
            let pulse_r = 1.0 + (t * 2.0).sin() * 0.3;
            let base_size = 120.0;
            renderer.draw(&Sprite::new(mat)
                .with_position(vec2(
                    cx + angle_r.cos() * orbit_r,
                    cy + angle_r.sin() * orbit_r,
                ))
                .with_scale(vec2(base_size * pulse_r, base_size * pulse_r))
                .with_angle(Angle::Radians(angle_r))
                .with_color(Color { r: 0.8, g: 0.2, b: 0.2, a: 0.7 })
                .with_z_index(0));

            // Blue rect — orbits counter-clockwise, opposite phase.
            let angle_b = -t * 1.2 + std::f32::consts::PI;
            let pulse_b = 1.0 + (t * 2.0 + std::f32::consts::PI).sin() * 0.3;
            renderer.draw(&Sprite::new(mat)
                .with_position(vec2(
                    cx + angle_b.cos() * orbit_r,
                    cy + angle_b.sin() * orbit_r,
                ))
                .with_scale(vec2(base_size * pulse_b, base_size * pulse_b))
                .with_angle(Angle::Radians(angle_b))
                .with_color(Color { r: 0.2, g: 0.5, b: 0.8, a: 0.7 })
                .with_z_index(1));

            // Green rect — bounces vertically in the background.
            let bounce_y = cy + 220.0 + (t * 3.0).sin() * 60.0;
            let bounce_x = cx + (t * 0.8).sin() * 100.0;
            renderer.draw(&Sprite::new(mat)
                .with_position(vec2(bounce_x, bounce_y))
                .with_scale(vec2(80.0, 80.0))
                .with_color(Color { r: 0.2, g: 0.8, b: 0.3, a: 0.5 })
                .with_z_index(0));

            renderer.draw(&Sprite::new(mat)
                .with_position(vec2(120.0, 120.0))
                .with_color(Color::YELLOW)
                .with_scale(vec2(100.0, 100.0))
                .with_angle(Angle::Degrees(-t * 10.0))
            )
        }

        // Animated circles — pulsing radius and drifting position.
        if let Some(mat) = self.circle_material {
            // Central circle — pulsing.
            let pulse = 50.0 + (t * 4.0).sin() * 20.0;
            renderer.draw(&ShapeInstance::<CircleShape>::new(mat)
                .with_position(vec2(cx, cy))
                .with_scale(vec2(pulse * 2.0, pulse * 2.0))
                .with_color(Color { r: 1.0, g: 0.9, b: 0.2, a: 0.6 })
                .with_z_index(2));

            // Small orbiting circles.
            for i in 0..3 {
                let phase = t * 2.0 + i as f32 * std::f32::consts::TAU / 3.0;
                let r = 260.0;
                let x = cx + phase.cos() * r;
                let y = cy + phase.sin() * r;
                let size = 30.0 + (t * 5.0 + i as f32).sin() * 10.0;
                let col = match i {
                    0 => Color { r: 0.9, g: 0.3, b: 0.9, a: 0.8 },
                    1 => Color { r: 0.3, g: 0.9, b: 0.9, a: 0.8 },
                    _ => Color { r: 0.9, g: 0.7, b: 0.3, a: 0.8 },
                };
                renderer.draw(&ShapeInstance::<CircleShape>::new(mat)
                    .with_position(vec2(x, y))
                    .with_scale(vec2(size * 2.0, size * 2.0))
                    .with_color(col)
                    .with_z_index(3));
            }
        }

        renderer.end_scene(
            RenderPassDescriptor::new().with_color_clear(Color { r: 0.1, g: 0.1, b: 0.12, a: 1.0 }),
            &ctx.assets_manager,
        );
    }
}

impl ApplicationProxy for EditorApp {
    fn on_init(&mut self, ctx: &mut ApplicationContext) -> EngineResult<()> {
        // Create the off-screen texture target (800×600, same format as surface).
        let target = ctx.render_ctx.create_texture_target(
            TextureConfig {
                size: TextureSize::new_texture2d(800, 600),
                format: TextureFormat::Rgba8Unorm,
                label: "Scene render target".to_string(),
                sample_count: 4,
                ..TextureConfig::default()
            },
        );
        self.scene_target = Some(target);

        // Register the texture with egui so we can display it in the viewport panel.
        let target = self.scene_target.as_ref().unwrap();
        self.scene_texture = Some(ctx.register_texture(target, FilterMode::Linear));

        // Get the default sprite material (white texture) for rendering rectangles.
        self.sprite_material = Some(
            ctx.default_assets
                .expect::<SpriteMaterial>(Nova2dDefaults::DefaultSpriteMaterial),
        );

        // Get the default circle material for rendering circles.
        self.circle_material = Some(
            ctx.default_assets
                .expect::<nova::nova2d::materials::CircleMaterial>(Nova2dDefaults::DefaultCircleMaterial),
        );

        // ─── Async loading demo ───────────────────────────────────────
        // Dispatch an async texture load with a supplier closure. The closure
        // simulates slow work (50ms sleep) so we can observe the Loading state
        // for several frames before it transitions to Ready.
        let strong = ctx.assets_manager.load(|| {
            std::thread::sleep(std::time::Duration::from_millis(500000));
            Ok(Texture::from_raw(
                vec![0xFF, 0x00, 0xFF, 0xFF], // magenta 1×1
                TextureConfig {
                    size: TextureSize::new_texture2d(1, 1),
                    label: "Async demo texture".to_string(),
                    ..TextureConfig::default()
                },
                SamplerConfig::default(),
            ))
        });
        self.async_texture_weak = Some(strong.as_handle());
        self.async_texture = Some(strong);

        Ok(())
    }

    fn on_update(&mut self, ctx: &mut ApplicationContext, _dt: Duration) {
        self.load_frames = self.load_frames.saturating_add(1);

        // Poll the async texture state.
        if let Some(weak) = self.async_texture_weak {
            let state = ctx.assets_manager.get_asset(weak);
            match state {
                AssetState::Loading => {}
                AssetState::Ready(_) => {
                }
                AssetState::Failed(err) => {
                    eprintln!("Async texture load failed: {err}");
                }
                AssetState::Empty => {
                    // Slot was off-loaded after we dropped the StrongHandle.
                    self.async_texture_weak = None;
                    self.offloaded_count += 1;
                }
            }
        }

        // Track off-load count. cleanup_offloaded is called by the engine
        // after on_update each frame. We detect the off-load via the
        // AssetState::Empty transition above.
    }

    fn on_render(&mut self, ctx: &ApplicationContext, _frame: &mut Frame) {
        // Render the 2D scene into the off-screen texture.
        // This happens before egui paints, so the texture is ready for display.
        self.render_scene(ctx);
    }

    fn on_gui(&mut self, ui: &mut egui::Ui) {
        let tex_id = self.scene_texture.as_ref().map(|h| h.id);
        DockArea::new(&mut self.dock_state)
            .style(Style::from_egui(ui.style().as_ref()))
            .show_inside(ui, &mut EditorTabViewer { scene_texture_id: tex_id });

        egui::Window::new("Data Window")
        .show(ui, |ui| {

            // ─── Async loading + off-loading demo panel ─────────────────────
            ui.separator();
            ui.heading("Asset System Demo");
            ui.spacing();
    
            let weak = self.async_texture_weak;
            let has_strong = self.async_texture.is_some();
    
            match (weak, has_strong) {
                (Some(_weak), true) => {
                    // The AssetsManager is behind ApplicationContext — we can't access
                    // it from on_gui. Poll asset_state via on_update instead.
                    // For the demo, we show frame count and handle status.
                    ui.label(format!("Frames since load: {}", self.load_frames));
                    ui.label(format!("Off-loaded count: {}", self.offloaded_count));
    
                    ui.label(if self.load_frames < 4 {
                        "State: Loading…"
                    } else {
                        "State: Ready (asset available)"
                    });
    
                    ui.spacing();
                    if ui.button("Drop StrongHandle (trigger off-load)").clicked() {
                        // Dropping the StrongHandle removes the last strong ref.
                        // Next frame's cleanup_offloaded will recycle the slot.
                        self.async_texture = None;
                        //ui.ctx().request_repaint();
                    }
                }
                (Some(_weak), false) => {
                    ui.label("StrongHandle dropped — waiting for off-load…");
                    ui.label(format!("Frames since load: {}", self.load_frames));
                    ui.label(format!("Off-loaded count: {}", self.offloaded_count));
                }
                (None, _) => {
                    ui.label("No async asset loaded.");
                    ui.label(format!("Frames since load: {}", self.load_frames));
                    ui.label(format!("Off-loaded count: {}", self.offloaded_count));
                }
            }
        });
    }
}

fn main() -> EngineResult<()> {
    println!("=== Nova Engine — Editor ===");

    ApplicationBuilder::new(EditorApp::new())
        .alter_window_attributes(|config| {
            config.with_size((1000, 800))
            .with_title("Nova editor")
        })
        .with_plugins(DefaultPlugins)
        .build()
        .run()
}
