
use nova::{core::{
    EngineResult, app::{ApplicationBuilder, ApplicationContext, ApplicationProxy}, graphics::{
        buffer::{VertexBufferLayout, VertexFormat}, color::Color, environment::{EnvironmentDescriptor, EnvironmentUniform}, frame::Frame, material::{Material, MaterialMetadata, MaterialTemplate, MaterialTemplateMetadata}, render_pass::RenderPassDescriptor, sampler::{Sampler, SamplerMetadata}, shader::{Shader, ShaderEntryPoint, ShaderMetadata, ShaderStage}, texture::{SamplerBindingType, Texture, TextureBinding, TextureMetadata, TextureSize, TextureViewDimension}, uniform::UniformValue,
    }, math::{Vec2, vec2}, window::LogicalSize,
}, nova2d::{camera::Camera2D, quad::Quad, render2d::Render2D}};

pub struct AppProxy {
    material: Option<nova::core::assets::handle::Handle<Material>>,
}

impl AppProxy {
    pub fn new() -> Self {
        Self { material: None }
    }
}

impl ApplicationProxy for AppProxy {
    fn on_update(&mut self, _ctx: &mut ApplicationContext, _dt: std::time::Duration) {
    }

    fn on_render(&mut self, ctx: &ApplicationContext, frame: &mut Frame) {
        let material_handle = self.material.expect("material loaded in on_init");

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
        );

        renderer.end_scene(
            RenderPassDescriptor::new(),
            &ctx.assets_manager
        );

    }

    fn on_init(&mut self, ctx: &mut ApplicationContext) -> EngineResult<()> {
        // Load the shader (inline, embedded from the WGSL file).
        let shader = ctx.assets_manager.load::<Shader>(ShaderMetadata::from_inline(
            "quad_shader",
            include_str!("../assets/shader.wgsl"),
            ShaderEntryPoint::Both {
                vs_entry_point: "vs_main".into(),
                fs_entry_point: "fs_main".into(),
            },
        ))?;

        let sampler = ctx.assets_manager.load::<Sampler>(SamplerMetadata::default())?;

        let texture = ctx.assets_manager.load::<Texture>(TextureMetadata::from_raw(
            "White texture",
            vec![0xFF, 0xFF, 0xFF, 0xFF],
            TextureSize::new_texture2d(1, 1),
            sampler
        ))?;

        // Material template: one vertex attribute (position: Float32x2),
        // no blending, no depth, triangle list, one fragment uniform (color),
        // no textures.
        let template = ctx.assets_manager.load::<MaterialTemplate>(
            MaterialTemplateMetadata {
                vertex_shader: shader,
                fragment_shader: Some(shader),
                buffer_layout: VertexBufferLayout::new(&[
                    VertexFormat::Float32x4,
                    VertexFormat::Float32x2,
                    VertexFormat::Float32x4
                ], 
                    0
                ),
                blend_state: Default::default(),
                depth_stencil: None,
                topology: Default::default(),
                uniform_layout: vec![],
                texture_layout: vec![
                    TextureBinding {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sampler_binding_type: SamplerBindingType::Filtering,
                        texture_binding_slot: 0,
                        sample_binding_slot: 1,
                    }
                ],
            },
        )?;

        // Material instance: red quad.
        let material = ctx.assets_manager.load::<Material>(
            MaterialMetadata::new(template)
            .with_texture(0, texture)
        )?;

        self.material = Some(material);

        Ok(())
    }
}

fn main() -> EngineResult<()> {
    simple_logger::init_with_env().unwrap();

    ApplicationBuilder::new(AppProxy::new())
        .alter_window_attributes(|win_attr| 
            win_attr.with_inner_size(LogicalSize::new(800, 600))
        )
        .build()
        .run()
}
