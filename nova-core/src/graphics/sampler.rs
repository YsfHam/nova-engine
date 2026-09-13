use std::hash::{Hash, Hasher};

/// Engine-native sampler configuration. Not an asset — samplers are now
/// renderer-managed, cached by `SamplerConfig` in `RenderContext`.
///
/// Derives `Hash + Eq + PartialEq` so it can be used as a `HashMap` key
/// for the sampler cache. `f32` fields are hashed via `to_bits()` and
/// compared via `to_bits()` since `f32` doesn't implement `Eq`/`Hash`.
#[derive(Clone, Debug)]
pub struct SamplerConfig {
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

impl PartialEq for SamplerConfig {
    fn eq(&self, other: &Self) -> bool {
        self.address_mode_u == other.address_mode_u
            && self.address_mode_v == other.address_mode_v
            && self.address_mode_w == other.address_mode_w
            && self.mag_filter == other.mag_filter
            && self.min_filter == other.min_filter
            && self.mipmap_filter == other.mipmap_filter
            && self.lod_min_clamp.to_bits() == other.lod_min_clamp.to_bits()
            && self.lod_max_clamp.to_bits() == other.lod_max_clamp.to_bits()
            && self.compare == other.compare
            && self.anisotropy_clamp == other.anisotropy_clamp
            && self.border_color == other.border_color
            && self.label == other.label
    }
}

impl Eq for SamplerConfig {}

impl Hash for SamplerConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.address_mode_u.hash(state);
        self.address_mode_v.hash(state);
        self.address_mode_w.hash(state);
        self.mag_filter.hash(state);
        self.min_filter.hash(state);
        self.mipmap_filter.hash(state);
        self.lod_min_clamp.to_bits().hash(state);
        self.lod_max_clamp.to_bits().hash(state);
        self.compare.hash(state);
        self.anisotropy_clamp.hash(state);
        self.border_color.hash(state);
        self.label.hash(state);
    }
}

impl Default for SamplerConfig {
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

impl SamplerConfig {
    /// Creates a `wgpu::SamplerDescriptor` from this config.
    pub fn descriptor(&self) -> wgpu::SamplerDescriptor<'_> {
        wgpu::SamplerDescriptor {
            label: Some(&self.label),
            address_mode_u: self.address_mode_u.into(),
            address_mode_v: self.address_mode_v.into(),
            address_mode_w: self.address_mode_w.into(),
            mag_filter: self.mag_filter.into(),
            min_filter: self.min_filter.into(),
            mipmap_filter: self.mipmap_filter.into(),
            lod_min_clamp: self.lod_min_clamp,
            lod_max_clamp: self.lod_max_clamp,
            compare: self.compare.map(Into::into),
            anisotropy_clamp: self.anisotropy_clamp,
            border_color: self.border_color.map(Into::into),
        }
    }
}

/// Engine-native mirror of `wgpu::AddressMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AddressMode {
    ClampToEdge,
    Repeat,
    MirrorRepeat,
    ClampToBorder,
}

/// Engine-native mirror of `wgpu::FilterMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FilterMode {
    Nearest,
    Linear,
}

/// Engine-native mirror of `wgpu::CompareFunction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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