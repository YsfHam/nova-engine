use std::path::PathBuf;

use image::ImageReader;

use crate::{
    assets::{Asset, error::AssetError, handle::Handle, load::AssetLoader}, graphics::{render::RenderContext, sampler::Sampler},
};

/// A GPU texture. References a shared [`Sampler`] via a [`Handle`].
///
/// The `texture` field is kept for ownership: the `wgpu::TextureView` borrows
/// the underlying `wgpu::Texture`, so the texture must live as long as the
/// view. It isn't read directly after construction, hence `allow(dead_code)`.
#[allow(dead_code)]
pub struct Texture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: Handle<Sampler>,
    metadata: TextureMetadata,
}

impl Texture {
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn sampler_handle(&self) -> Handle<Sampler> {
        self.sampler
    }
}

/// Engine-native texture dimension. Mirrors `wgpu::TextureDimension`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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

#[derive(Copy, Clone, Debug)]
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
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TextureFormat {
    Rgba8UnormSrgb,
    Bgra8UnormSrgb,
    R8Unorm,
    R32Float,
    Rgba32Float,
}

impl From<TextureFormat> for wgpu::TextureFormat {
    fn from(f: TextureFormat) -> Self {
        match f {
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
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
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

/// Where texture pixel data comes from.
#[derive(Clone, Debug)]
pub enum TextureSource {
    File(PathBuf),
    /// Raw in-memory pixel data plus its dimensions. Used for procedurally
    /// generated textures (e.g. the default 1×1 white texture in Step 9).
    Raw { data: Vec<u8>, size: TextureSize},
}

/// Runtime metadata for a [`Texture`]. Carries a [`Handle<Sampler>`] for the
/// dependency on a [`Sampler`] asset.
///
/// For the on-disk form (where the sampler dependency is a relative path to
/// a sampler metadata file) see `TextureMetadataFile` — that type will be
/// defined when serialization lands. `load_from_file` converts the file form
/// into this runtime form.
#[derive(Clone, Debug)]
pub struct TextureMetadata {
    pub source: TextureSource,
    pub format: TextureFormat,
    pub mip_level_count: u32,
    pub sample_count: u32,
    pub usage: TextureUsages,
    pub label: String,
    pub sampler: Handle<Sampler>,
}

impl TextureMetadata {
    /// Convenience constructor for the common case: load an image file with
    /// sensible defaults (RGBA8 sRGB, 1 mip, single-sample, texture-binding +
    /// copy-dst usage). The caller must supply a sampler handle.
    pub fn from_file(path: impl Into<PathBuf>, sampler: Handle<Sampler>) -> Self {
        let path = path.into();
        let label = path
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| "Texture".to_string());
        Self {
            source: TextureSource::File(path),
            format: TextureFormat::Rgba8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            label,
            sampler,
        }
    }

    /// Convenience constructor for an in-memory (procedural) texture.
    pub fn from_raw(
        label: impl Into<String>,
        data: Vec<u8>,
        size: TextureSize,
        sampler: Handle<Sampler>,
    ) -> Self {
        Self {
            source: TextureSource::Raw { data, size },
            format: TextureFormat::Rgba8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            label: label.into(),
            sampler,
        }
    }
}

impl Asset for Texture {
    type Metadata = TextureMetadata;

    fn metadata(&self) -> &Self::Metadata {
        &self.metadata
    }
}

pub struct TextureLoader;

impl AssetLoader for TextureLoader {
    type Asset = Texture;

    fn load(
        &self,
        metadata: TextureMetadata,
        ctx: &crate::assets::load::LoadContext,
    ) -> Result<Texture, AssetError> {
        let (data, size) = match &metadata.source {
            TextureSource::File(path) => {
                let image = ImageReader::open(path)?.decode()?.to_rgba8();
                let (w, h) = image.dimensions();
                (image.into_raw(), TextureSize::new_texture2d(w, h))
            }
            TextureSource::Raw { data, size } => {
                (data.clone(), *size)
            }
        };

        let texture = Self::build_texture(&ctx.render_ctx.get(), &data, size, &metadata);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Ok(Texture {
            texture,
            view,
            sampler: metadata.sampler,
            metadata,
        })
    }
}

impl TextureLoader {
    fn build_texture(
        render_ctx: &RenderContext,
        data: &[u8],
        texture_size: TextureSize,
        metadata: &TextureMetadata,
    ) -> wgpu::Texture {

        let texture = render_ctx.device().create_texture(&wgpu::TextureDescriptor {
            label: Some(&metadata.label),
            size: texture_size.into(),
            mip_level_count: metadata.mip_level_count,
            sample_count: metadata.sample_count,
            dimension: texture_size.tex_dim.into(),
            format: metadata.format.into(),
            usage: metadata.usage.into(),
            view_formats: &[],
        });

        render_ctx.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * texture_size.width),
                rows_per_image: Some(texture_size.height),
            },
            texture_size.into(),
        );
        texture
    }
}

/// Engine-native texture view dimension. Mirrors `wgpu::TextureViewDimension`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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

/// Engine-native sampler binding type. Mirrors `wgpu::SamplerBindingType`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SamplerBindingType {
    Filtering,
    NonFiltering,
    Comparison,
}

impl From<SamplerBindingType> for wgpu::SamplerBindingType {
    fn from(t: SamplerBindingType) -> Self {
        match t {
            SamplerBindingType::Filtering => wgpu::SamplerBindingType::Filtering,
            SamplerBindingType::NonFiltering => wgpu::SamplerBindingType::NonFiltering,
            SamplerBindingType::Comparison => wgpu::SamplerBindingType::Comparison,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TextureBinding {
    pub multisampled: bool,
    pub view_dimension: TextureViewDimension,
    pub sampler_binding_type: SamplerBindingType,
    pub texture_binding_slot: u32,
    pub sample_binding_slot: u32,
}

impl TextureBinding {
    pub fn is_filterable(&self) -> bool {
        self.sampler_binding_type == SamplerBindingType::Filtering
    }
}