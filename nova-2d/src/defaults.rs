use nova_core::{assets::{defaults::DefaultAssetsKey, handle::Handle}, graphics::{sampler::SamplerConfig, texture::{Texture, TextureConfig, TextureSize}}};

use crate::materials::{SpriteMaterial};

/// A 1×1 white RGBA8 texture — the default texture for sprite materials.
pub fn default_white_texture() -> Texture {
    Texture::from_raw(
        vec![255, 255, 255, 255],
        TextureConfig {
            size: TextureSize::new_texture2d(1, 1),
            ..TextureConfig::default()
        },
        SamplerConfig::default(),
    )
}

/// A default sprite material using the white texture.
pub fn default_sprite_material(white_texture: Handle<Texture>) -> SpriteMaterial {
    SpriteMaterial::new(white_texture)
}

pub enum Nova2dDefaults {
    WhiteTexture,
    DefaultSpriteMaterial,
    DefaultCircleMaterial,
}

impl DefaultAssetsKey for Nova2dDefaults {
    fn as_str(&self) -> &'static str {
        match self {
            Nova2dDefaults::WhiteTexture => "WhiteTexture",
            Nova2dDefaults::DefaultSpriteMaterial => "DefaultSpriteMaterial",
            Nova2dDefaults::DefaultCircleMaterial => "DefaultCircleMaterial",
        }
    }
}