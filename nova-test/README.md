# nova-test

`nova-test` is the executable integration harness for Nova. It exercises the public facade instead of reaching into private renderer implementation details.

## Current Scope

The main example:

- creates an application through `ApplicationBuilder`;
- installs `DefaultPlugins`;
- retrieves default materials from `DefaultAssets`;
- loads CPU textures and inserts them into `AssetsManager`;
- creates `SpriteMaterial` assets and a `SpriteAtlas`;
- builds a `Camera2D` environment bind group;
- renders an animated atlas sprite through `Render2D`.

`stress.rs` contains a high-volume sprite workload with several material, tint, rotation, UV, and batching patterns. It records draw, submission, and total render timings while ramping toward a configured sprite-count cliff.

## Running

```text
cargo run --release -p nova-test
```

The harness is a manual visual and performance check, not a substitute for deterministic unit or integration tests.

## Planned Scope

- Make demo asset paths portable and package the required runtime assets with the harness.
- Add separate examples for colored sprites, textured sprites, atlas animation, off-screen rendering, and custom materials.
- Add deterministic checks for stale handles, material registration, bind-group validation, and cache reuse.
- Add benchmark commands with reproducible scene sizes and machine-readable timing output.
- Add a minimal 3D smoke scene when the 3D renderer is available.