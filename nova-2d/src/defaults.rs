use nova_core::{app::ApplicationContext, assets::{defaults::DefaultAssetsKey, error::AssetError, handle::Handle}, graphics::{buffer::{InstanceBufferLayout, VertexBufferLayout, VertexFormat}, material::{BlendMode, Material, MaterialMetadata, MaterialTemplate, MaterialTemplateMetadata}, sampler::{FilterMode, SamplerMetadata}, shader::{Shader, ShaderEntryPoint, ShaderMetadata}, texture::{SamplerBindingType, Texture, TextureBinding, TextureMetadata, TextureViewDimension}}};

pub fn create_material_with_texture_meta(
    ctx: &mut ApplicationContext, 
    texture_meta: TextureMetadata) 
    -> Result<Handle<Material>, AssetError> {
    let texture = ctx.assets_manager.load(texture_meta)?;
    create_material_with_texture(ctx, texture)
}

pub fn create_material_with_texture(
    ctx: &mut ApplicationContext, 
    texture: Handle<Texture>) 
    -> Result<Handle<Material>, AssetError> {
    let template = 
        ctx
        .default_assets
        .expect::<MaterialTemplate>(Nova2dDefaults::TexturedQuadMaterialTemplate);

    ctx.assets_manager
    .load(
        MaterialMetadata::new(template)
        .with_texture(0, texture)
    )
}

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

/// Material template for the instanced 2D quad renderer.
///
/// Vertex buffer (slot 0, step mode Vertex): `BaseVertex2D` — position only
/// (Float32x2). Lives in the shared geometry buffer (uploaded once).
///
/// Instance buffer (slot 1, step mode Instance): `InstanceData2D` — transform
/// (mat3x3 = 3×Float32x3), color (Float32x4), uv_rect (Float32x4). Uploaded
/// per-frame as instance data.
pub(crate) fn default_material_template(shader: Handle<Shader>) -> MaterialTemplateMetadata {
    MaterialTemplateMetadata {
        vertex_shader: shader,
        fragment_shader: Some(shader),
        buffer_layout: VertexBufferLayout::new(&[VertexFormat::Float32x2], 0),
        instance_layout: Some(InstanceBufferLayout::new(
            &[VertexFormat::Float32x3, VertexFormat::Float32x3, VertexFormat::Float32x3, VertexFormat::Float32x4, VertexFormat::Float32x4],
            1, // location_offset: vertex buffer uses location 0, instance starts at 1
        )),
        blend_state: BlendMode::Alpha,
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

pub(crate) fn pixelated_sampler() -> SamplerMetadata {
    SamplerMetadata {
        mag_filter: FilterMode::Nearest,
        ..Default::default()
    }
}

pub enum Nova2dDefaults {
    TexturedQuadShader,
    TexturedQuadMaterialTemplate,
    WhiteTextureMaterial,
    PixelatedSampler,
}

impl DefaultAssetsKey for Nova2dDefaults {
    fn as_str(&self) -> &'static str {
        match self {
            Nova2dDefaults::TexturedQuadShader => "TexturedQuadShader",
            Nova2dDefaults::TexturedQuadMaterialTemplate => "TexturedQuadMaterialTemplate",
            Nova2dDefaults::WhiteTextureMaterial => "WhiteTextureMaterial",
            Nova2dDefaults::PixelatedSampler => "PixelatedSampler",
        }
    }
}