use nova_core::{
    assets::Asset, graphics::{
        buffer::{InstanceBufferLayout, VertexBufferLayout, VertexFormat}, material::{AsBindGroup, BindGroup, BindGroupLayout, BlendMode, Material, MaterialTemplate}, shader::{ShaderEntryPoint, ShaderInfo, ShaderSource},
    },
};

/// A material for rendering filled circles. Has no bind group data — the
/// circle's color and radius are per-instance (`CircleInstance`), so the
/// material itself is stateless.
pub struct CircleMaterial;

impl CircleMaterial {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CircleMaterial {
    fn default() -> Self {
        Self::new()
    }
}

impl Asset for CircleMaterial {}

impl AsBindGroup for CircleMaterial {
    fn as_bind_group(&self) -> BindGroup {
        // No bindings — color and radius come from instance data.
        BindGroup::new()
    }
}

impl Material for CircleMaterial {
    fn template() -> MaterialTemplate {
        MaterialTemplate {
            shader: ShaderInfo {
                source: ShaderSource::Inline(include_str!("../../assets/circle_shader.wgsl").to_string()),
                entry_point: ShaderEntryPoint::Both {
                    vs_entry_point: "vs_main".into(),
                    fs_entry_point: "fs_main".into(),
                },
            },
            buffer_layout: VertexBufferLayout::new(&[VertexFormat::Float32x2], 0),
            instance_layout: Some(InstanceBufferLayout::new(
                &[
                    VertexFormat::Float32x2,  // position
                    VertexFormat::Float32,    // radius
                    VertexFormat::Float32x4,  // color
                ],
                1,
            )),
            blend_state: BlendMode::Alpha,
            depth_stencil: None,
            topology: Default::default(),
            bind_group_layout: BindGroupLayout::new(),
        }
    }
}