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