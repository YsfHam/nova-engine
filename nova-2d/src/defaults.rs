use nova_core::{assets::{defaults::DefaultAssetsKey, handle::Handle}, graphics::{buffer::{VertexBufferLayout, VertexFormat}, material::{MaterialMetadata, MaterialTemplate, MaterialTemplateMetadata}, shader::{Shader, ShaderEntryPoint, ShaderMetadata}, texture::{SamplerBindingType, Texture, TextureBinding, TextureViewDimension}}};

pub(crate) fn default_shader() -> ShaderMetadata {
    ShaderMetadata::from_inline(
        "quad_shader",
        include_str!("../assets/shader.wgsl"),
        ShaderEntryPoint::Both {
            vs_entry_point: "vs_main".into(),
            fs_entry_point: "fs_main".into(),
        },
    )
}

pub(crate) fn default_material_template(shader: Handle<Shader>) -> MaterialTemplateMetadata {
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
    }
}

pub(crate) fn default_material(material_template: Handle<MaterialTemplate>, white_texture: Handle<Texture>) -> MaterialMetadata {
    MaterialMetadata::new(material_template)
    .with_texture(0, white_texture)
}

pub enum Nova2dDefaults {
    TexturedQuadShader,
    TexturedQuadMaterialTemplate,
    WhiteTextureMaterial,
}

impl DefaultAssetsKey for Nova2dDefaults {
    fn as_str(&self) -> &'static str {
        match self {
            Nova2dDefaults::TexturedQuadShader => "TexturedQuadShader",
            Nova2dDefaults::TexturedQuadMaterialTemplate => "TexturedQuadMaterialTemplate",
            Nova2dDefaults::WhiteTextureMaterial => "WhiteTextureMaterial",
        }
    }
}