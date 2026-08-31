
use std::collections::HashMap;

use crate::{
    assets::{Asset, error::AssetError, handle::Handle, load::{AssetLoader, LoadContext}},
    graphics::{
        buffer::BufferLayout,
        shader::Shader,
        texture::Texture,
        uniform::{UniformBinding, UniformValue},
    },
};

pub struct MaterialTemplate {
    metadata: MaterialTemplateMetadata,
}

impl MaterialTemplate {
    pub fn new(metadata: MaterialTemplateMetadata) -> Self {
        Self { metadata }
    }

    /// Handle to the vertex shader asset.
    pub fn vertex_shader(&self) -> Handle<Shader> {
        self.metadata.vertex_shader
    }

    /// Handle to the fragment shader asset.
    pub fn fragment_shader(&self) -> Option<Handle<Shader>> {
        self.metadata.fragment_shader
    }

    pub fn buffer_layout(&self) -> &BufferLayout {
        &self.metadata.buffer_layout
    }

    /// The optional instance buffer layout (for GPU-side instancing).
    pub fn instance_layout(&self) -> Option<&BufferLayout> {
        self.metadata.instance_layout.as_ref()
    }

    pub fn blend_state(&self) -> BlendMode {
        self.metadata.blend_state
    }

    pub fn depth_stencil(&self) -> Option<&DepthStencilConfig> {
        self.metadata.depth_stencil.as_ref()
    }

    pub fn topology(&self) -> Topology {
        self.metadata.topology
    }

    pub fn uniform_layout(&self) -> &[UniformBinding] {
        &self.metadata.uniform_layout
    }

    pub fn texture_layout(&self) -> &[crate::graphics::texture::TextureBinding] {
        &self.metadata.texture_layout
    }
}

impl Asset for MaterialTemplate {
    type Metadata = MaterialTemplateMetadata;

    fn metadata(&self) -> &Self::Metadata {
        &self.metadata
    }
}

// ──────────────────────────────────────────────────────────────────────────
//  MaterialTemplateMetadata — serializable description of a template.
//
//  Uses engine-native enums (not raw `wgpu` types) so it stays serializable
//  and `Clone + Send + Sync`, consistent with the Step 6 `SamplerMetadata`
//  pattern. The template/loader translates these to `wgpu` at pipeline-build
//  time.
// ──────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct MaterialTemplateMetadata {
    pub vertex_shader: Handle<Shader>,
    pub fragment_shader: Option<Handle<Shader>>,
    pub buffer_layout: BufferLayout,
    /// Optional instance buffer layout (step mode `Instance`). When present,
    /// the pipeline declares a second vertex buffer at slot 1 for per-instance
    /// data. Enables GPU-side instancing.
    pub instance_layout: Option<BufferLayout>,
    pub blend_state: BlendMode,
    pub depth_stencil: Option<DepthStencilConfig>,
    pub topology: Topology,
    /// Declares the uniform bindings the template's shaders expect. Drives
    /// bind group layout creation and material load-time validation.
    pub uniform_layout: Vec<UniformBinding>,
    pub texture_layout: Vec<crate::graphics::texture::TextureBinding>,
}

// ──────────────────────────────────────────────────────────────────────────
//  Loader — builds a MaterialTemplate from its metadata.
//
//  The metadata already carries resolved `Handle<Shader>`s for its shader
//  dependencies (whoever constructed the metadata resolved them, e.g. by
//  calling `ctx.assets.load::<Shader>(...)` first). So the loader's job is
//  simply to wrap the metadata in a `MaterialTemplate` — no nested loading
//  is needed here. This keeps the loader trivial.
// ──────────────────────────────────────────────────────────────────────────

pub struct MaterialTemplateLoader;

impl AssetLoader for MaterialTemplateLoader {
    type Asset = MaterialTemplate;

    fn load(
        &self,
        metadata: MaterialTemplateMetadata,
        _ctx: &LoadContext,
    ) -> Result<MaterialTemplate, AssetError> {
        Ok(MaterialTemplate::new(metadata))
    }
}

/// Blend configuration. `None`-equivalent is `BlendMode::None` (no blending,
/// opaque rendering). Kept as a non-`Option` enum for simpler serialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    /// No blending — pixels overwrite the target.
    None,
    /// Standard alpha blending: `src * src.a + dst * (1 - src.a)`.
    Alpha,
    /// Additive blending: `src + dst`.
    Additive,
}

impl Default for BlendMode {
    fn default() -> Self {
        BlendMode::None
    }
}

impl From<BlendMode> for Option<wgpu::BlendState> {
    fn from(m: BlendMode) -> Self {
        match m {
            BlendMode::None => None,
            BlendMode::Alpha => Some(wgpu::BlendState::ALPHA_BLENDING),
            BlendMode::Additive => Some(wgpu::BlendState::ADDITIVE),
        }
    }
}

/// Depth/stencil configuration. Engine-native mirror of the subset of
/// `wgpu::DepthStencilState` we care about serializing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DepthStencilConfig {
    pub format: DepthFormat,
    pub depth_compare: DepthCompare,
    pub depth_write_enabled: bool,
    // Stencil support can be added here when needed (Step 12 / 3D).
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepthFormat {
    Depth24Plus,
    Depth24PlusStencil8,
    Depth32Float,
}

impl From<DepthFormat> for wgpu::TextureFormat {
    fn from(f: DepthFormat) -> Self {
        match f {
            DepthFormat::Depth24Plus => wgpu::TextureFormat::Depth24Plus,
            DepthFormat::Depth24PlusStencil8 => wgpu::TextureFormat::Depth24PlusStencil8,
            DepthFormat::Depth32Float => wgpu::TextureFormat::Depth32Float,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepthCompare {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

impl From<DepthCompare> for wgpu::CompareFunction {
    fn from(c: DepthCompare) -> Self {
        match c {
            DepthCompare::Never => wgpu::CompareFunction::Never,
            DepthCompare::Less => wgpu::CompareFunction::Less,
            DepthCompare::Equal => wgpu::CompareFunction::Equal,
            DepthCompare::LessEqual => wgpu::CompareFunction::LessEqual,
            DepthCompare::Greater => wgpu::CompareFunction::Greater,
            DepthCompare::NotEqual => wgpu::CompareFunction::NotEqual,
            DepthCompare::GreaterEqual => wgpu::CompareFunction::GreaterEqual,
            DepthCompare::Always => wgpu::CompareFunction::Always,
        }
    }
}

impl From<&DepthStencilConfig> for wgpu::DepthStencilState {
    fn from(c: &DepthStencilConfig) -> Self {
        // wgpu 30 models both depth write and depth compare as optional:
        // `None` disables that aspect. Our config keeps them enabled.
        wgpu::DepthStencilState {
            format: c.format.into(),
            depth_write_enabled: Some(c.depth_write_enabled),
            depth_compare: Some(c.depth_compare.into()),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }
    }
}

/// Primitive topology for the draw call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Topology {
    PointList,
    LineList,
    LineStrip,
    TriangleList,
    TriangleStrip,
}

impl Default for Topology {
    fn default() -> Self {
        Topology::TriangleList
    }
}

impl From<Topology> for wgpu::PrimitiveTopology {
    fn from(t: Topology) -> Self {
        match t {
            Topology::PointList => wgpu::PrimitiveTopology::PointList,
            Topology::LineList => wgpu::PrimitiveTopology::LineList,
            Topology::LineStrip => wgpu::PrimitiveTopology::LineStrip,
            Topology::TriangleList => wgpu::PrimitiveTopology::TriangleList,
            Topology::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
//  Material — an immutable per-instance reference to a template plus the
//  per-instance uniform values and texture bindings. Many materials share
//  one template (and thus one pipeline).
//
//  Materials are immutable: once loaded from metadata, they cannot be
//  changed. To change a material, load a new one with new metadata. This
//  removes the need for a `dirty` flag and keeps the metadata-is-identity
//  invariant (important for future serialization, dedup, hot-reload).
//
//  The uniform values are stored typed (`HashMap<String, UniformValue>`) so
//  the material is serializable. The GPU uniform buffer is a derived cache
//  built by `MaterialUniformPool` / `BindGroupAllocator` — not stored here.
// ──────────────────────────────────────────────────────────────────────────

/// An immutable material instance: a template handle plus the uniform
/// values and texture bindings that vary between instances of the same
/// template.
#[derive(Clone)]
pub struct Material {
    metadata: MaterialMetadata,
}

impl Material {
    pub fn template(&self) -> Handle<MaterialTemplate> {
        self.metadata.template
    }

    pub fn texture(&self, binding: u32) -> Option<Handle<Texture>> {
        self.metadata.textures.get(&binding).copied()
    }

    pub fn textures(&self) -> &HashMap<u32, Handle<Texture>> {
        &self.metadata.textures
    }

    pub fn uniforms(&self) -> &HashMap<String, UniformValue> {
        &self.metadata.uniforms
    }
}

impl Asset for Material {
    type Metadata = MaterialMetadata;

    fn metadata(&self) -> &Self::Metadata {
        &self.metadata
    }
}

/// The metadata for a [`Material`]. Fully describes an immutable material
/// instance: its template, uniform values (typed, keyed by name), and texture
/// bindings (keyed by binding slot). This is the serialization source of truth
/// — the GPU buffer and bind group are derived caches, not part of the
/// metadata.
#[derive(Clone)]
pub struct MaterialMetadata {
    template: Handle<MaterialTemplate>,
    uniforms: HashMap<String, UniformValue>,
    textures: HashMap<u32, Handle<Texture>>,
}

impl MaterialMetadata {
    pub fn new(template: Handle<MaterialTemplate>) -> Self {
        Self {
            template,
            uniforms: HashMap::new(),
            textures: HashMap::new(),
        }
    }

    /// Builder: add a uniform value. The name must match a uniform declared
    /// in the template's `uniform_layout`; this is validated at load time.
    pub fn with_uniform(mut self, name: impl Into<String>, value: UniformValue) -> Self {
        self.uniforms.insert(name.into(), value);
        self
    }

    /// Builder: set a texture binding. The binding slot must match a texture
    /// binding declared in the template's `texture_layout`; validated at load.
    pub fn with_texture(mut self, binding: u32, texture: Handle<Texture>) -> Self {
        self.textures.insert(binding, texture);
        self
    }

    pub fn template(&self) -> Handle<MaterialTemplate> {
        self.template
    }

    pub fn uniforms(&self) -> &HashMap<String, UniformValue> {
        &self.uniforms
    }

    pub fn textures(&self) -> &HashMap<u32, Handle<Texture>> {
        &self.textures
    }
}

// ──────────────────────────────────────────────────────────────────────────
//  MaterialLoader — validates the material against its template at load time.
//
//  The loader resolves the `MaterialTemplate` via `ctx.assets.get_asset`,
//  then validates every uniform and texture binding in the metadata against
//  the template's layout. Validation failures produce
//  `AssetError::DependencyValidationFailure` with the material name, the
//  template name, and a human-readable reason. This ensures that by the time
//  a `Material` exists in the asset store, it is guaranteed to match its
//  template — draw-time never needs to handle mismatches.
// ──────────────────────────────────────────────────────────────────────────

pub struct MaterialLoader;

impl AssetLoader for MaterialLoader {
    type Asset = Material;

    fn load(
        &self,
        metadata: MaterialMetadata,
        ctx: &LoadContext<'_>,
    ) -> Result<Material, AssetError> {
        let template = ctx
            .assets
            .get_asset::<MaterialTemplate>(metadata.template)
            .ok_or_else(|| AssetError::DependencyValidationFailure {
                asset_name: "Material".to_string(),
                dependency_name: "MaterialTemplate".to_string(),
                reason: format!(
                    "material template handle {:?} could not be resolved — the template must be loaded before the material",
                    metadata.template
                ),
            })?;

        // Validate uniforms: every uniform declared in the template must be
        // provided with a value of the correct type, and no extra uniforms
        // are allowed.
        for binding in template.uniform_layout() {
            match metadata.uniforms.get(&binding.name) {
                None => {
                    return Err(AssetError::DependencyValidationFailure {
                        asset_name: "Material".to_string(),
                        dependency_name: "MaterialTemplate".to_string(),
                        reason: format!(
                            "uniform `{}` is declared in the template but not provided in the material metadata",
                            binding.name
                        ),
                    });
                }
                Some(value) => {
                    if value.ty() != binding.ty {
                        return Err(AssetError::DependencyValidationFailure {
                            asset_name: "Material".to_string(),
                            dependency_name: "MaterialTemplate".to_string(),
                            reason: format!(
                                "uniform `{}` type mismatch: template expects {:?}, material provides {:?}",
                                binding.name,
                                binding.ty,
                                value.ty()
                            ),
                        });
                    }
                }
            }
        }

        for name in metadata.uniforms.keys() {
            if !template.uniform_layout().iter().any(|b| b.name == *name) {
                return Err(AssetError::DependencyValidationFailure {
                    asset_name: "Material".to_string(),
                    dependency_name: "MaterialTemplate".to_string(),
                    reason: format!(
                        "uniform `{}` is provided in the material but not declared in the template",
                        name
                    ),
                });
            }
        }

        // Validate textures: every texture binding in the template must be
        // provided, and no extra bindings are allowed.
        for tex_binding in template.texture_layout() {
            if !metadata.textures.contains_key(&tex_binding.texture_binding_slot) {
                return Err(AssetError::DependencyValidationFailure {
                    asset_name: "Material".to_string(),
                    dependency_name: "MaterialTemplate".to_string(),
                    reason: format!(
                        "texture binding slot {} is declared in the template but not provided in the material metadata",
                        tex_binding.texture_binding_slot
                    ),
                });
            }
        }

        for slot in metadata.textures.keys() {
            if !template.texture_layout().iter().any(|b| b.texture_binding_slot == *slot) {
                return Err(AssetError::DependencyValidationFailure {
                    asset_name: "Material".to_string(),
                    dependency_name: "MaterialTemplate".to_string(),
                    reason: format!(
                        "texture binding slot {} is provided in the material but not declared in the template",
                        slot
                    ),
                });
            }
        }

        Ok(Material { metadata })
    }
}