use nova_core::{
    assets::{Asset, handle::Handle}, graphics::{
        buffer::{InstanceBufferLayout, VertexBufferLayout, VertexFormat}, color::Color, material::{AsBindGroup, BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BlendMode, Material, MaterialTemplate}, shader::{ShaderEntryPoint, ShaderInfo, ShaderSource, ShaderStage}, texture::{Texture, TextureSampleType, TextureViewDimension},
    },
};

pub struct SpriteMaterial {
    texture: Handle<Texture>,
    tint: Color,
}

impl SpriteMaterial {
    pub fn new(texture: Handle<Texture>) -> Self {
        Self {
            texture,
            tint: Color::WHITE,
        }
    }

    pub fn texture(&self) -> Handle<Texture> {
        self.texture
    }

    pub fn tint(&self) -> Color {
        self.tint
    }

    pub fn set_tint(&mut self, tint: Color) {
        self.tint = tint;
    }
}

impl Asset for SpriteMaterial {}

impl AsBindGroup for SpriteMaterial {
    fn as_bind_group(&self) -> BindGroup {
        BindGroup::new()
            .with_entry(BindGroupEntry::Texture {
                binding_slot: 0,
                sampler_binding_slot: 1,
                visibility: ShaderStage::Fragment,
                texture: self.texture,
                view_dimension: TextureViewDimension::D2,
                sample_type: TextureSampleType::FloatFilterable,
            })
    }
}

impl Material for SpriteMaterial {
    fn template() -> MaterialTemplate {
        MaterialTemplate {
            shader: ShaderInfo {
                source: ShaderSource::Inline(include_str!("../../assets/sprite_shader.wgsl").to_string()),
                entry_point: ShaderEntryPoint::Both {
                    vs_entry_point: "vs_main".into(),
                    fs_entry_point: "fs_main".into(),
                },
            },
            buffer_layout: VertexBufferLayout::new(&[VertexFormat::Float32x2], 0),
            instance_layout: Some(InstanceBufferLayout::new(
                &[
                    VertexFormat::Float32x2, 
                    VertexFormat::Float32x2,
                    VertexFormat::Float32x4,
                    VertexFormat::Float32x4,
                    VertexFormat::Float32
                ],
                1,
            )),
            blend_state: BlendMode::Alpha,
            depth_stencil: None,
            topology: Default::default(),
            bind_group_layout: BindGroupLayout::new().with_entry(BindGroupLayoutEntry::Texture {
                binding_slot: 0,
                sampler_binding_slot: 1,
                visibility: ShaderStage::Fragment,
                view_dimension: TextureViewDimension::D2,
                sample_type: TextureSampleType::FloatFilterable,
            }),
        }
    }
}