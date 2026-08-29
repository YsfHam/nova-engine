//! Memory-layout re-exports.
//!
//! nova-core uses [`bytemuck`] for zero-copy casting between Rust values and
//! byte slices (uniform buffer writes, vertex uploads). This module is the
//! single re-export point so engine users never depend on `bytemuck`
//! directly.
//!
//! Re-exports the traits most commonly needed (`Pod`, `Zeroable`) and the
//! `bytemuck::cast_slice` / `bytemuck::bytes_of` helpers used for GPU uploads.
//! The `derive` feature stays on `bytemuck` in `Cargo.toml` so downstream
//! crates can derive `Pod`/`Zeroable` after re-importing these traits.
//!
//! ```ignore
//! use nova_core::mem::{Pod, Zeroable, cast_slice};
//! ```

pub use bytemuck;