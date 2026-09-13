use nova_core::{
    assets::Asset, graphics::{
        buffer::{InstanceBufferLayout, VertexBufferLayout, VertexFormat}, color::Color, material::{AsBindGroup, BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BlendMode, Material, MaterialTemplate}, shader::{ShaderEntryPoint, ShaderInfo, ShaderSource, ShaderStage}, uniform::UniformValue,
    }, math::Vec4,
};

pub struct ColorMaterial {
    color: Color,
}

impl ColorMaterial {
    pub fn new(color: Color) -> Self {
        Self { color }
    }

    pub fn color(&self) -> Color {
        self.color
    }

    pub fn set_color(&mut self, color: Color) {
        self.color = color;
    }
}

impl Asset for ColorMaterial {}

impl AsBindGroup for ColorMaterial {
    fn as_bind_group(&self) -> BindGroup {
        BindGroup::new().with_entry(BindGroupEntry::Uniform {
            binding_slot: 0,
            visibility: ShaderStage::Fragment,
            value: UniformValue::Vec4(Vec4::from_array(self.color.into())),
        })
    }
}

impl Material for ColorMaterial {
    fn template() -> MaterialTemplate {
        MaterialTemplate {
            shader: ShaderInfo {
                source: ShaderSource::Inline(include_str!("../../assets/color_shader.wgsl").to_string()),
                entry_point: ShaderEntryPoint::Both {
                    vs_entry_point: "vs_main".into(),
                    fs_entry_point: "fs_main".into(),
                },
            },
            buffer_layout: VertexBufferLayout::new(&[VertexFormat::Float32x2], 0),
            instance_layout: Some(InstanceBufferLayout::new(
                &[VertexFormat::Float32x3, VertexFormat::Float32x3, VertexFormat::Float32x3, VertexFormat::Float32x4, VertexFormat::Float32x4],
                1,
            )),
            blend_state: BlendMode::Alpha,
            depth_stencil: None,
            topology: Default::default(),
            bind_group_layout: BindGroupLayout::new().with_entry(BindGroupLayoutEntry::Uniform {
                binding_slot: 0,
                visibility: ShaderStage::Fragment,
                ty: nova_core::graphics::uniform::UniformType::Vec4,
            }),
        }
    }
}