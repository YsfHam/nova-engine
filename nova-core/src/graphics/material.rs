
use std::hash::Hash;

use crate::{
    assets::{Asset, handle::WeakHandle}, graphics::{
        buffer::BufferLayout, shader::{ShaderInfo, ShaderStage}, texture::{Texture, TextureSampleType, TextureViewDimension}, uniform::{UniformType, UniformValue},
    },
};

/// Describes a material's pipeline configuration: shader, buffer layouts,
/// blend/depth/topology, and bind group layout. Returned by
/// `Material::template()`. Not an asset — it's a plain owned struct.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MaterialTemplate {
    pub shader: ShaderInfo,
    pub buffer_layout: BufferLayout,
    pub instance_layout: Option<BufferLayout>,
    pub blend_state: BlendMode,
    pub depth_stencil: Option<DepthStencilConfig>,
    pub topology: Topology,
    pub bind_group_layout: BindGroupLayout,
}

/// A material is an asset that also provides a static template describing
/// its pipeline, and can produce a `BindGroup` from its instance data.
pub trait Material: Asset + AsBindGroup + Send + Sync + 'static {
    fn template() -> MaterialTemplate;
}

/// Produces a `BindGroup` (owned, no GPU access) from a material's instance
/// data. The renderer calls this each frame to build the per-material bind
/// group.
pub trait AsBindGroup {
    fn as_bind_group(&self) -> BindGroup;
}

/// A layout-only description of a bind group: binding slots, visibility,
/// and types — no runtime values. Used by `MaterialTemplate` and as a
/// pipeline cache key component. Derives `Hash` so the template hash can
/// include it directly.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BindGroupLayout {
    pub entries: Vec<BindGroupLayoutEntry>,
}

impl BindGroupLayout {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn with_entry(mut self, entry: BindGroupLayoutEntry) -> Self {
        self.entries.push(entry);
        self
    }

    /// Creates a `wgpu::BindGroupLayout` from this layout's entries.
    /// Returns `None` if the layout has no entries.
    pub fn create_layout(&self, device: &wgpu::Device, label: &str) -> Option<wgpu::BindGroupLayout> {
        let entries = self.layout_entries();
        if entries.is_empty() {
            return None;
        }
        Some(device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &entries,
        }))
    }

    /// Produces the `wgpu::BindGroupLayoutEntry`s for this layout.
    /// Uniform entries become buffer bindings; texture entries become
    /// a texture binding + a sampler binding.
    fn layout_entries(&self) -> Vec<wgpu::BindGroupLayoutEntry> {
        self.entries
            .iter()
            .flat_map(|entry| match entry {
                BindGroupLayoutEntry::Uniform {
                    binding_slot,
                    visibility,
                    ty,
                } => {
                    vec![wgpu::BindGroupLayoutEntry {
                        binding: *binding_slot,
                        visibility: (*visibility).into(),
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: std::num::NonZeroU64::new(ty.size()),
                        },
                        count: None,
                    }]
                }
                BindGroupLayoutEntry::Texture {
                    binding_slot,
                    sampler_binding_slot,
                    visibility,
                    view_dimension,
                    sample_type,
                } => {
                    vec![
                        wgpu::BindGroupLayoutEntry {
                            binding: *binding_slot,
                            visibility: (*visibility).into(),
                            ty: wgpu::BindingType::Texture {
                                sample_type: (*sample_type).into(),
                                view_dimension: (*view_dimension).into(),
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: *sampler_binding_slot,
                            visibility: (*visibility).into(),
                            ty: wgpu::BindingType::Sampler((*sample_type).into()),
                            count: None,
                        },
                    ]
                }
            })
            .collect()
    }
}

/// A single entry in a [`BindGroupLayout`]. Either a uniform binding
/// (described by type, not value) or a texture binding (which implies a
/// paired sampler binding).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BindGroupLayoutEntry {
    Uniform {
        binding_slot: u32,
        visibility: ShaderStage,
        ty: UniformType,
    },
    Texture {
        binding_slot: u32,
        sampler_binding_slot: u32,
        visibility: ShaderStage,
        view_dimension: TextureViewDimension,
        sample_type: TextureSampleType,
    },
}

/// An owned, GPU-free description of a bind group's runtime values. Contains
/// only owned/copied data (Vec, u32, enums, GenericHandle) — no borrows, no
/// wgpu types. The layout is described separately by [`BindGroupLayout`].
pub struct BindGroup {
    pub entries: Vec<BindGroupEntry>,
}

impl BindGroup {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn with_entry(mut self, entry: BindGroupEntry) -> Self {
        self.entries.push(entry);
        self
    }
}

/// A single entry in a [`BindGroup`]. Either a uniform value or a texture
/// binding (which implies a paired sampler binding). Holds runtime values;
/// the layout is described by [`BindGroupLayoutEntry`].
pub enum BindGroupEntry {
    Uniform {
        binding_slot: u32,
        visibility: ShaderStage,
        value: UniformValue,
    },
    Texture {
        binding_slot: u32,
        sampler_binding_slot: u32,
        visibility: ShaderStage,
        texture: WeakHandle<Texture>,
        view_dimension: TextureViewDimension,
        sample_type: TextureSampleType,
    },
}

/// Blend configuration. `None`-equivalent is `BlendMode::None` (no blending,
/// opaque rendering). Kept as a non-`Option` enum for simpler serialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendMode {
    /// No blending — pixels overwrite the target.
    None,
    /// Standard alpha blending: `src * src.a + dst * (1 - src.a)`.
    Alpha,
    /// Additive blending: `src + dst`.
    Additive,
}

impl Default for BlendMode {
    fn default() -> Self {
        BlendMode::None
    }
}

impl From<BlendMode> for Option<wgpu::BlendState> {
    fn from(m: BlendMode) -> Self {
        match m {
            BlendMode::None => None,
            BlendMode::Alpha => Some(wgpu::BlendState::ALPHA_BLENDING),
            BlendMode::Additive => Some(wgpu::BlendState::ADDITIVE),
        }
    }
}

/// Depth/stencil configuration. Engine-native mirror of the subset of
/// `wgpu::DepthStencilState` we care about serializing.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DepthStencilConfig {
    pub format: DepthFormat,
    pub depth_compare: DepthCompare,
    pub depth_write_enabled: bool,
    // Stencil support can be added here when needed (Step 12 / 3D).
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DepthFormat {
    Depth24Plus,
    Depth24PlusStencil8,
    Depth32Float,
}

impl From<DepthFormat> for wgpu::TextureFormat {
    fn from(f: DepthFormat) -> Self {
        match f {
            DepthFormat::Depth24Plus => wgpu::TextureFormat::Depth24Plus,
            DepthFormat::Depth24PlusStencil8 => wgpu::TextureFormat::Depth24PlusStencil8,
            DepthFormat::Depth32Float => wgpu::TextureFormat::Depth32Float,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DepthCompare {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

impl From<DepthCompare> for wgpu::CompareFunction {
    fn from(c: DepthCompare) -> Self {
        match c {
            DepthCompare::Never => wgpu::CompareFunction::Never,
            DepthCompare::Less => wgpu::CompareFunction::Less,
            DepthCompare::Equal => wgpu::CompareFunction::Equal,
            DepthCompare::LessEqual => wgpu::CompareFunction::LessEqual,
            DepthCompare::Greater => wgpu::CompareFunction::Greater,
            DepthCompare::NotEqual => wgpu::CompareFunction::NotEqual,
            DepthCompare::GreaterEqual => wgpu::CompareFunction::GreaterEqual,
            DepthCompare::Always => wgpu::CompareFunction::Always,
        }
    }
}

impl From<&DepthStencilConfig> for wgpu::DepthStencilState {
    fn from(c: &DepthStencilConfig) -> Self {
        // wgpu 30 models both depth write and depth compare as optional:
        // `None` disables that aspect. Our config keeps them enabled.
        wgpu::DepthStencilState {
            format: c.format.into(),
            depth_write_enabled: Some(c.depth_write_enabled),
            depth_compare: Some(c.depth_compare.into()),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }
    }
}

/// Primitive topology for the draw call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Topology {
    PointList,
    LineList,
    LineStrip,
    TriangleList,
    TriangleStrip,
}

impl Default for Topology {
    fn default() -> Self {
        Topology::TriangleList
    }
}

impl From<Topology> for wgpu::PrimitiveTopology {
    fn from(t: Topology) -> Self {
        match t {
            Topology::PointList => wgpu::PrimitiveTopology::PointList,
            Topology::LineList => wgpu::PrimitiveTopology::LineList,
            Topology::LineStrip => wgpu::PrimitiveTopology::LineStrip,
            Topology::TriangleList => wgpu::PrimitiveTopology::TriangleList,
            Topology::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
        }
    }
}