use nova_core::{
    EngineResult, app::{ApplicationBuilder, ApplicationContext, ApplicationProxy}, assets::resolve::ResolvedMaterial, graphics::{
        buffer::VertexBufferLayout, color::Color, environment::{EnvironmentDescriptor, EnvironmentUniform}, material::{Material, MaterialMetadata, MaterialTemplate, MaterialTemplateMetadata}, render_pass::RenderPassDescriptor, render_target::RenderTarget, shader::{Shader, ShaderEntryPoint, ShaderMetadata, ShaderStage}, uniform::{MaterialUniformEntry, UniformBinding, UniformType, UniformValue},
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

    fn on_render(&mut self, ctx: &ApplicationContext, target: &mut RenderTarget<'_>) {
        let material_handle = self.material.expect("material loaded in on_init");

        let mut commander = target.commander(
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

        let scene_bind_group = commander
            .build_scene_bind_group()
            .expect("scene uniforms uploaded");

        // Build the material uniform pool once (first frame). Materials are
        // immutable, so the pool is stable afterwards.
        if !commander.is_uniform_pool_built() {
            let material = ctx
                .assets_manager
                .get_asset(material_handle)
                .expect("material asset");
            let template = ctx
                .assets_manager
                .get_asset(material.template())
                .expect("material template asset");
            commander.build_uniform_pool([MaterialUniformEntry {
                handle: material_handle,
                material,
                template,
            }]);
        }

        // Resolve the material (template + shaders + textures).
        let resolved_material =
            ResolvedMaterial::new(material_handle, &ctx.assets_manager)
                .expect("material resolves");

        // Begin the pass, compile the pipeline, build the bind group, and
        // draw — all in one call. `draw_material` split-borrows the encoder
        // and the RenderContext caches (disjoint fields) so no wgpu handles
        // are cloned.
        commander.draw_material(
            RenderPassDescriptor::default().with_color_clear(Color::BLACK),
            &scene_bind_group,
            resolved_material,
            0..6,
            0..1,
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

        // Material template: no vertex buffer (quad generated in shader),
        // no blending, no depth, triangle list, one fragment uniform (color),
        // no textures.
        let template = ctx.assets_manager.load::<MaterialTemplate>(
            MaterialTemplateMetadata {
                vertex_shader: shader,
                fragment_shader: Some(shader),
                vertex_buffer_layout: VertexBufferLayout::new(&[]),
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
