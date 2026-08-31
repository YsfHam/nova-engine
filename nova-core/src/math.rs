//! Math re-exports.
//!
//! nova-core standardizes on [`glam`] for vector and matrix math. This module
//! is the single re-export point so engine users (and `nova-test`) never need
//! to depend on `glam` directly — they pull math types from `nova_core::math`.
//!
//! ```ignore
//! use nova_core::math::{Mat4, Vec4};
//! ```

pub use glam::*;

#[derive(Copy, Clone)]
pub enum Angle {
    Radians(f32),
    Degrees(f32),
}

impl Angle {
    pub const ZERO: Self = Self::Radians(0.0);
}

impl From<f32> for Angle {
    fn from(value: f32) -> Self {
        Self::Radians(value)
    }
}

impl From<Angle> for f32 {
    fn from(value: Angle) -> Self {
        match value {
            Angle::Radians(angle) => angle,
            Angle::Degrees(angle) => angle.to_radians(),
        }
    }
}