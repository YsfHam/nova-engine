use std::path::PathBuf;

use image::ImageReader;

use crate::{
    assets::Asset,
    graphics::sampler::SamplerConfig,
};

/// A CPU-only texture asset. Holds raw pixel data and configuration — no
/// wgpu types. The renderer uploads this to a `GpuTexture` on demand and
/// caches it in `RenderContext`.
///
/// The `config` field (`TextureConfig`) describes the GPU texture resource
/// (size, format, usage, etc.). The `sampler_config` field describes how the
/// texture is sampled (filtering, address mode, etc.). They are separate so a
/// texture can be created without a sampler and vice-versa.
pub struct Texture {
    data: Vec<u8>,
    config: TextureConfig,
    sampler_config: SamplerConfig,
}

impl Asset for Texture {}

impl Texture {
    /// Decodes an image file into a CPU `Texture` with the given config and
    /// sampler config. The image dimensions override `config.size` — the
    /// texture is always sized to match the decoded image.
    pub fn from_file(
        path: impl Into<PathBuf>,
        mut config: TextureConfig,
        sampler_config: SamplerConfig,
    ) -> Result<Self, std::io::Error> {
        let path = path.into();
        let image = ImageReader::open(&path)?
            .decode()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
            .to_rgba8();
        let (w, h) = image.dimensions();
        config.size = TextureSize::new_texture2d(w, h);
        Ok(Self {
            data: image.into_raw(),
            config,
            sampler_config,
        })
    }

    /// Creates a CPU `Texture` from raw pixel data. The size is taken from
    /// `config.size` — it must be set correctly before calling this.
    pub fn from_raw(data: Vec<u8>, config: TextureConfig, sampler_config: SamplerConfig) -> Self {
        Self {
            data,
            config,
            sampler_config,
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn size(&self) -> TextureSize {
        self.config.size
    }

    pub fn config(&self) -> &TextureConfig {
        &self.config
    }

    pub fn sampler_config(&self) -> &SamplerConfig {
        &self.sampler_config
    }
}

/// Configuration for a texture: format, usage, sampler, etc. Stored in the
/// CPU `Texture` asset and used by the renderer to create the `GpuTexture`.
#[derive(Clone, Debug)]
pub struct TextureConfig {
    pub size: TextureSize,
    pub format: TextureFormat,
    pub mip_level_count: u32,
    pub sample_count: u32,
    pub usage: TextureUsages,
    pub label: String,
}

impl Default for TextureConfig {
    fn default() -> Self {
        Self {
            size: TextureSize::new_texture2d(1, 1),
            format: TextureFormat::Rgba8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            label: "Texture".to_string(),
        }
    }
}

impl<'a> From<&'a TextureConfig> for wgpu::TextureDescriptor<'a> {
    fn from(config: &'a TextureConfig) -> Self {
        wgpu::TextureDescriptor {
            label: Some(&config.label),
            size: config.size.into(),
            mip_level_count: config.mip_level_count,
            sample_count: config.sample_count,
            dimension: config.size.tex_dim.into(),
            format: config.format.into(),
            usage: config.usage.into(),
            view_formats: &[],
        }
    }
}

/// Renderer-owned GPU texture: the wgpu resource triple. Cached in
/// `RenderContext` by `GenericHandle` (the texture asset's handle).
pub(crate) struct GpuTexture {
    view: wgpu::TextureView,
}

impl GpuTexture {
    /// Creates a GPU texture from pixel data. Writes `data` into the texture
    /// immediately via the queue.
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &[u8],
        config: &TextureConfig,
    ) -> Self {
        let size = config.size;
        let texture = device.create_texture(&config.into());

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * size.width()),
                rows_per_image: Some(size.height()),
            },
            size.into(),
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self { view }
    }

    /// Creates an empty GPU texture (no pixel data uploaded). The texture is
    /// allocated with the size and format from `config`.
    ///
    /// Used internally by `TextureRenderTarget` to create MSAA textures.
    pub(crate) fn empty(device: &wgpu::Device, config: &TextureConfig) -> Self {
        let texture = device.create_texture(&config.into());
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self { view }
    }

    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

}

/// Engine-native texture dimension. Mirrors `wgpu::TextureDimension`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TextureDimension {
    D1,
    D2,
    D3,
}

impl From<TextureDimension> for wgpu::TextureDimension {
    fn from(d: TextureDimension) -> Self {
        match d {
            TextureDimension::D1 => wgpu::TextureDimension::D1,
            TextureDimension::D2 => wgpu::TextureDimension::D2,
            TextureDimension::D3 => wgpu::TextureDimension::D3,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TextureSize {
    width: u32,
    height: u32,
    depth: u32,
    tex_dim: TextureDimension,
}

impl TextureSize {
    pub fn new_texture2d(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            depth: 1,
            tex_dim: TextureDimension::D2,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

impl Into<wgpu::Extent3d> for TextureSize {
    fn into(self) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: self.width,
            height: self.height,
            depth_or_array_layers: self.depth,
        }
    }
}

/// Engine-native texture format. A serializable mirror of the subset of
/// `wgpu::TextureFormat` we expose. Extend as new formats are needed.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TextureFormat {
    Rgba8Unorm,
    Rgba8UnormSrgb,
    Bgra8UnormSrgb,
    R8Unorm,
    R32Float,
    Rgba32Float,
}

impl From<TextureFormat> for wgpu::TextureFormat {
    fn from(f: TextureFormat) -> Self {
        match f {
            TextureFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            TextureFormat::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
            TextureFormat::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,
            TextureFormat::R8Unorm => wgpu::TextureFormat::R8Unorm,
            TextureFormat::R32Float => wgpu::TextureFormat::R32Float,
            TextureFormat::Rgba32Float => wgpu::TextureFormat::Rgba32Float,
        }
    }
}

bitflags::bitflags! {
    /// Engine-native texture usage flags. A serializable mirror of
    /// `wgpu::TextureUsages`.
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct TextureUsages: u32 {
        const COPY_SRC = 1 << 0;
        const COPY_DST = 1 << 1;
        const TEXTURE_BINDING = 1 << 2;
        const STORAGE_BINDING = 1 << 3;
        const RENDER_ATTACHMENT = 1 << 4;
    }
}

impl From<TextureUsages> for wgpu::TextureUsages {
    fn from(u: TextureUsages) -> Self {
        let mut bits = wgpu::TextureUsages::empty();
        if u.contains(TextureUsages::COPY_SRC) {
            bits |= wgpu::TextureUsages::COPY_SRC;
        }
        if u.contains(TextureUsages::COPY_DST) {
            bits |= wgpu::TextureUsages::COPY_DST;
        }
        if u.contains(TextureUsages::TEXTURE_BINDING) {
            bits |= wgpu::TextureUsages::TEXTURE_BINDING;
        }
        if u.contains(TextureUsages::STORAGE_BINDING) {
            bits |= wgpu::TextureUsages::STORAGE_BINDING;
        }
        if u.contains(TextureUsages::RENDER_ATTACHMENT) {
            bits |= wgpu::TextureUsages::RENDER_ATTACHMENT;
        }
        bits
    }
}

/// Engine-native texture view dimension. Mirrors `wgpu::TextureViewDimension`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TextureViewDimension {
    D1,
    D2,
    D2Array,
    Cube,
    CubeArray,
    D3,
}

impl From<TextureViewDimension> for wgpu::TextureViewDimension {
    fn from(d: TextureViewDimension) -> Self {
        match d {
            TextureViewDimension::D1 => wgpu::TextureViewDimension::D1,
            TextureViewDimension::D2 => wgpu::TextureViewDimension::D2,
            TextureViewDimension::D2Array => wgpu::TextureViewDimension::D2Array,
            TextureViewDimension::Cube => wgpu::TextureViewDimension::Cube,
            TextureViewDimension::CubeArray => wgpu::TextureViewDimension::CubeArray,
            TextureViewDimension::D3 => wgpu::TextureViewDimension::D3,
        }
    }
}

/// Engine-native texture sample type. Determines how a texture is sampled
/// in the bind group layout (filterable float, non-filterable, depth, etc.).
/// Replaces the old `SamplerBindingType`. Used in `BindGroupEntry::Texture`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TextureSampleType {
    /// Float, filterable (standard color textures with linear filtering).
    FloatFilterable,
    /// Float, non-filterable (e.g. depth textures used as data).
    FloatNonFilterable,
    /// Non-filterable (integer textures, data textures).
    NonFiltering,
    /// Comparison sampling (depth comparison samplers).
    Comparison,
}

impl TextureSampleType {
    /// Returns `true` if this sample type is filterable.
    pub fn is_filterable(&self) -> bool {
        matches!(self, TextureSampleType::FloatFilterable)
    }
}

impl From<TextureSampleType> for wgpu::TextureSampleType {
    fn from(t: TextureSampleType) -> Self {
        match t {
            TextureSampleType::FloatFilterable => wgpu::TextureSampleType::Float { filterable: true },
            TextureSampleType::FloatNonFilterable => wgpu::TextureSampleType::Float { filterable: false },
            TextureSampleType::NonFiltering => wgpu::TextureSampleType::Sint,
            TextureSampleType::Comparison => wgpu::TextureSampleType::Depth,
        }
    }
}

impl From<TextureSampleType> for wgpu::SamplerBindingType {
    fn from(t: TextureSampleType) -> Self {
        match t {
            TextureSampleType::FloatFilterable => wgpu::SamplerBindingType::Filtering,
            TextureSampleType::FloatNonFilterable
            | TextureSampleType::NonFiltering => wgpu::SamplerBindingType::NonFiltering,
            TextureSampleType::Comparison => wgpu::SamplerBindingType::Comparison,
        }
    }
}