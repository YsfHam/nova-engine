use std::path::PathBuf;

use image::ImageReader;

use crate::{
    assets::{Asset, error::AssetError, handle::Handle, load::AssetLoader},
    graphics::{render::RenderContext, sampler::Sampler},
};

/// A GPU texture. References a shared [`Sampler`] via a [`Handle`].
pub struct Texture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: Handle<Sampler>,
    metadata: TextureMetadata,
}

impl Texture {
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn sampler_handle(&self) -> Handle<Sampler> {
        self.sampler
    }
}


#[derive(Copy, Clone, Debug)]
pub struct TextureSize {
    width: u32,
    height: u32,
    depth: u32,
    tex_dim: wgpu::TextureDimension,
}

impl TextureSize {
    pub fn new_texture2d(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            depth: 1,
            tex_dim: wgpu::TextureDimension::D2
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
    pub format: wgpu::TextureFormat,
    pub mip_level_count: u32,
    pub sample_count: u32,
    pub usage: wgpu::TextureUsages,
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
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
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
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
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
        &mut self,
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

        let texture = Self::build_texture(&ctx.render_ctx.borrow(), &data, size, &metadata);
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
            dimension: texture_size.tex_dim,
            format: metadata.format,
            usage: metadata.usage,
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