use crate::assets::{Asset, error::AssetError, load::AssetLoader};

/// A GPU sampler. Shared across textures via `Handle<Sampler>`.
///
/// Samplers are assets so they can be reused (many textures can reference the
/// same sampler) and so they participate in the metadata-driven load/serialize
/// pipeline like any other asset.
pub struct Sampler {
    sampler: wgpu::Sampler,
    metadata: SamplerMetadata,
}

impl Sampler {
    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }
}

/// Engine-native sampler configuration. Stored inside [`Sampler`] and used as
/// its identity. Kept free of `wgpu` types that are not `Clone + Send + Sync`
/// so the metadata remains serializable.
#[derive(Clone, Debug)]
pub struct SamplerMetadata {
    pub address_mode_u: AddressMode,
    pub address_mode_v: AddressMode,
    pub address_mode_w: AddressMode,
    pub mag_filter: FilterMode,
    pub min_filter: FilterMode,
    pub mipmap_filter: FilterMode,
    pub lod_min_clamp: f32,
    pub lod_max_clamp: f32,
    pub compare: Option<CompareFunction>,
    pub anisotropy_clamp: u16,
    pub border_color: Option<SamplerBorderColor>,
    pub label: String,
}

impl Default for SamplerMetadata {
    fn default() -> Self {
        Self {
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
            label: "Sampler".to_string(),
        }
    }
}

/// Engine-native mirror of `wgpu::AddressMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressMode {
    ClampToEdge,
    Repeat,
    MirrorRepeat,
    ClampToBorder,
}

/// Engine-native mirror of `wgpu::FilterMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterMode {
    Nearest,
    Linear,
}

/// Engine-native mirror of `wgpu::CompareFunction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompareFunction {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

/// Engine-native mirror of `wgpu::SamplerBorderColor`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SamplerBorderColor {
    TransparentBlack,
    OpaqueBlack,
    OpaqueWhite,
}

impl From<AddressMode> for wgpu::AddressMode {
    fn from(m: AddressMode) -> Self {
        match m {
            AddressMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
            AddressMode::Repeat => wgpu::AddressMode::Repeat,
            AddressMode::MirrorRepeat => wgpu::AddressMode::MirrorRepeat,
            AddressMode::ClampToBorder => wgpu::AddressMode::ClampToBorder,
        }
    }
}

impl From<FilterMode> for wgpu::FilterMode {
    fn from(m: FilterMode) -> Self {
        match m {
            FilterMode::Nearest => wgpu::FilterMode::Nearest,
            FilterMode::Linear => wgpu::FilterMode::Linear,
        }
    }
}

impl From<FilterMode> for wgpu::MipmapFilterMode {
    fn from(m: FilterMode) -> Self {
        match m {
            FilterMode::Nearest => wgpu::MipmapFilterMode::Nearest,
            FilterMode::Linear => wgpu::MipmapFilterMode::Linear,
        }
    }
}

impl From<CompareFunction> for wgpu::CompareFunction {
    fn from(c: CompareFunction) -> Self {
        match c {
            CompareFunction::Never => wgpu::CompareFunction::Never,
            CompareFunction::Less => wgpu::CompareFunction::Less,
            CompareFunction::Equal => wgpu::CompareFunction::Equal,
            CompareFunction::LessEqual => wgpu::CompareFunction::LessEqual,
            CompareFunction::Greater => wgpu::CompareFunction::Greater,
            CompareFunction::NotEqual => wgpu::CompareFunction::NotEqual,
            CompareFunction::GreaterEqual => wgpu::CompareFunction::GreaterEqual,
            CompareFunction::Always => wgpu::CompareFunction::Always,
        }
    }
}

impl From<SamplerBorderColor> for wgpu::SamplerBorderColor {
    fn from(c: SamplerBorderColor) -> Self {
        match c {
            SamplerBorderColor::TransparentBlack => wgpu::SamplerBorderColor::TransparentBlack,
            SamplerBorderColor::OpaqueBlack => wgpu::SamplerBorderColor::OpaqueBlack,
            SamplerBorderColor::OpaqueWhite => wgpu::SamplerBorderColor::OpaqueWhite,
        }
    }
}

impl Asset for Sampler {
    type Metadata = SamplerMetadata;

    fn metadata(&self) -> &Self::Metadata {
        &self.metadata
    }
}

pub struct SamplerLoader;

impl AssetLoader for SamplerLoader {
    type Asset = Sampler;

    fn load(
        &self,
        metadata: SamplerMetadata,
        ctx: &crate::assets::load::LoadContext,
    ) -> Result<Sampler, AssetError> {
        let render_ctx = ctx.render_ctx.get();
        let sampler = render_ctx.device().create_sampler(&wgpu::SamplerDescriptor {
            label: Some(&metadata.label),
            address_mode_u: metadata.address_mode_u.into(),
            address_mode_v: metadata.address_mode_v.into(),
            address_mode_w: metadata.address_mode_w.into(),
            mag_filter: metadata.mag_filter.into(),
            min_filter: metadata.min_filter.into(),
            mipmap_filter: metadata.mipmap_filter.into(),
            lod_min_clamp: metadata.lod_min_clamp,
            lod_max_clamp: metadata.lod_max_clamp,
            compare: metadata.compare.map(Into::into),
            anisotropy_clamp: metadata.anisotropy_clamp,
            border_color: metadata.border_color.map(Into::into),
        });

        Ok(Sampler { sampler, metadata })
    }
}