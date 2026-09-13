use nova_core::{assets::{defaults::DefaultAssetsKey, handle::Handle}, graphics::{color::Color, texture::{Texture, TextureConfig, TextureSize}}};

use crate::materials::{ColorMaterial, SpriteMaterial};

/// A 1×1 white RGBA8 texture — the default texture for sprite materials.
pub fn default_white_texture() -> Texture {
    Texture::from_raw(
        vec![255, 255, 255, 255],
        TextureSize::new_texture2d(1, 1),
        TextureConfig::default(),
    )
}

/// A default color material (white).
pub fn default_color_material() -> ColorMaterial {
    ColorMaterial::new(Color::WHITE)
}

/// A default sprite material using the white texture.
pub fn default_sprite_material(white_texture: Handle<Texture>) -> SpriteMaterial {
    SpriteMaterial::new(white_texture)
}

pub enum Nova2dDefaults {
    WhiteTexture,
    DefaultColorMaterial,
    DefaultSpriteMaterial,
}

impl DefaultAssetsKey for Nova2dDefaults {
    fn as_str(&self) -> &'static str {
        match self {
            Nova2dDefaults::WhiteTexture => "WhiteTexture",
            Nova2dDefaults::DefaultColorMaterial => "DefaultColorMaterial",
            Nova2dDefaults::DefaultSpriteMaterial => "DefaultSpriteMaterial",
        }
    }
}