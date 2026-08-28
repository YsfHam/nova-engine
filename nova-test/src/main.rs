use nova_core::{
    EngineResult, app::{ApplicationBuilder, ApplicationContext, ApplicationProxy}, graphics::{
        buffer::{VertexBufferLayout, VertexFormat}, color::Color, draw_batch::DrawBatch, environment::{EnvironmentDescriptor, EnvironmentUniform}, frame::Frame, material::{Material, MaterialMetadata, MaterialTemplate, MaterialTemplateMetadata}, render_pass::RenderPassDescriptor, shader::{Shader, ShaderEntryPoint, ShaderMetadata, ShaderStage}, uniform::{UniformBinding, UniformType, UniformValue},
    }, math::Vec4,
};

pub struct AppProxy {
    material: Option<nova_core::assets::handle::Handle<Material>>,
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

        let mut target = frame.render_target(&ctx.render_ctx);

        let commander = target.commander(
            EnvironmentDescriptor::new()
            .add_uniform(EnvironmentUniform {
                binding_slot: 0,
                visibilty: ShaderStage::Vertex,
                uniform: UniformValue::Mat4(nova_core::math::Mat4::IDENTITY),
            })
            .add_uniform(EnvironmentUniform {
                binding_slot: 1,
                visibilty: ShaderStage::Both,
                uniform: UniformValue::F32(0.0),
            })
        );

        // Quad geometry: 4 vertices (position: vec2<f32>), 6 u16 indices (2 tris).
        let vertices: [[f32; 2]; 4] = [
            [-0.5,  0.5],  // TL
            [-0.5, -0.5],  // BL
             [0.5, -0.5],  // BR
             [0.5,  0.5],  // TR
        ];
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let batch = DrawBatch::with_vertices(
            material_handle,
            &vertices,
            indices.to_vec(),
        );
        // Submit all batches in one render pass. The commander groups by
        // template (pipeline reuse) and merges contiguous same-material runs.
        commander.submit_batches(
            RenderPassDescriptor::default().with_color_clear(Color::BLACK),
            [batch],
            &ctx.assets_manager,
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

        // Material template: one vertex attribute (position: Float32x2),
        // no blending, no depth, triangle list, one fragment uniform (color),
        // no textures.
        let template = ctx.assets_manager.load::<MaterialTemplate>(
            MaterialTemplateMetadata {
                vertex_shader: shader,
                fragment_shader: Some(shader),
                buffer_layout: VertexBufferLayout::new(&[VertexFormat::Float32x2], 0),
                blend_state: Default::default(),
                depth_stencil: None,
                topology: Default::default(),
                uniform_layout: vec![UniformBinding {
                    name: "color".to_string(),
                    ty: UniformType::Vec4,
                    binding_slot: 0,
                    visibility: nova_core::graphics::shader::ShaderStage::Fragment,
                }],
                texture_layout: vec![],
            },
        )?;

        // Material instance: red quad.
        let material = ctx.assets_manager.load::<Material>(
            MaterialMetadata::new(template).with_uniform("color", UniformValue::Vec4(Vec4::new(1.0, 0.2, 0.2, 1.0))),
        )?;

        self.material = Some(material);

        Ok(())
    }
}

fn main() -> EngineResult<()> {
    simple_logger::init_with_env().unwrap();

    ApplicationBuilder::new(AppProxy::new())
        .build()
        .run()
}
