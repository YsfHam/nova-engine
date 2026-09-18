use std::ops::{Deref, DerefMut};

use nova_core::{
    assets::handle::WeakHandle, graphics::color::Color, math::{Angle, Vec2},
};

use crate::{
    materials::SpriteMaterial,
    shape::{RectangleShape, ShapeInstance},
    utils::RectF32,
};

pub struct Sprite {
    shape_instance: ShapeInstance<RectangleShape>,
}

impl Sprite {
    pub fn new(material: WeakHandle<SpriteMaterial>) -> Self {
        Self {
            shape_instance: ShapeInstance::new(RectangleShape::default(), material)
        }
    }

    pub fn with_position(mut self, position: Vec2) -> Self {
        self.position = position;
        self
    }

    pub fn with_angle(mut self, angle: Angle) -> Self {
        self.angle = angle;
        self
    }

    pub fn with_scale(mut self, scale: Vec2) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn with_uv(mut self, uv: RectF32) -> Self {
        self.uv = uv;
        self
    }

    pub fn with_z_index(mut self, z_index: u32) -> Self {
        self.shape_instance = self.shape_instance.with_z_index(z_index);
        self
    }
}

impl Deref for Sprite {
    type Target = ShapeInstance<RectangleShape>;

    fn deref(&self) -> &Self::Target {
        &self.shape_instance
    }
}

impl DerefMut for Sprite {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.shape_instance
    }
}

/// A grid-based sprite atlas: a texture divided into fixed-size cells.
///
/// All sprites share one [`GenericHandle`] (which binds the atlas texture).
/// Sprites are indexed by `u32` — row-major order (left-to-right, top-to-bottom).
pub struct SpriteAtlas {
    material: WeakHandle<SpriteMaterial>,
    /// Number of columns/rows in the grid (atlas_size / sprite_size).
    grid: Vec2,
    /// Pixel size of each cell. Used to compute UVs in pixel space first,
    /// then normalize — avoids floating-point error accumulation across rows
    /// (e.g. 1/6 = 0.1666... repeating in binary).
    cell_size: Vec2,
    /// Atlas texture dimensions in pixels. Used for UV normalization.
    atlas_size: Vec2,
}

impl SpriteAtlas {
    /// Creates a grid atlas from pixel dimensions.
    ///
    /// `atlas_size` is the texture dimensions in pixels. `sprite_size` is
    /// the dimensions of each cell in pixels. The atlas must divide evenly.
    pub fn new(material: WeakHandle<SpriteMaterial>, atlas_size: Vec2, sprite_size: Vec2) -> Self {
        let grid = atlas_size / sprite_size;
        Self {
            material,
            grid,
            cell_size: sprite_size,
            atlas_size,
        }
    }

    /// Returns the sprite at the given grid index (row-major order).
    ///
    /// Returns `None` if the index is out of bounds (exceeds the grid).
    pub fn sprite(&self, index: u32) -> Option<Sprite> {
        let cols = self.grid.x as u32;
        let rows = self.grid.y as u32;

        let col = index % cols;
        let row = index / cols;

        if col >= cols || row >= rows {
            return None;
        }

        let px_left = col as f32 * self.cell_size.x;
        let px_top = row as f32 * self.cell_size.y;
        let px_right = px_left + self.cell_size.x;
        let px_bottom = px_top + self.cell_size.y;

        // Normalize to 0..1 UV space.
        let left = px_left / self.atlas_size.x;
        let top = px_top / self.atlas_size.y;
        let right = px_right / self.atlas_size.x;
        let bottom = px_bottom / self.atlas_size.y;

        Some(Sprite::new(self.material)
            .with_uv(RectF32 { top, left, bottom, right }),
        )
    }

    /// Returns the sprite at the given grid coordinates (col, row).
    pub fn sprite_at(&self, col: u32, row: u32) -> Option<Sprite> {
        let cols = self.grid.x as u32;
        if col >= cols || row >= self.grid.y as u32 {
            return None;
        }
        self.sprite(row * cols + col)
    }
}