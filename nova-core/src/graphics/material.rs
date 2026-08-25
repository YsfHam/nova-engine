
use std::collections::HashMap;

use glam::{Mat4, Vec4};

use crate::{
    assets::{Asset, error::AssetError, handle::Handle, load::{AssetLoader, LoadContext}},
    graphics::{buffer::VertexBufferLayout, shader::Shader},
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
    pub fn fragment_shader(&self) -> Handle<Shader> {
        self.metadata.fragment_shader
    }

    pub fn vertex_buffer_layout(&self) -> &VertexBufferLayout {
        &self.metadata.vertex_buffer_layout
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

    /// A key that identifies the GPU pipeline this template compiles to.
    ///
    /// In Step 8 the `PipelineCache` will map `(PipelineKey, TextureFormat)`
    /// to a compiled `wgpu::RenderPipeline`. Because two materials share a
    /// pipeline iff they use the same template, the key is simply the
    /// template's `Handle` (already `Hash + Eq + Copy` from Step 5).
    pub fn pipeline_key(&self) -> PipelineKey {
        // The key is the *identity* of this template asset. We don't hash the
        // template's structural fields because two distinct template instances
        // with identical config should still be distinct keys (cheap, and
        // avoids hashing nested structs on every lookup). If dedup-by-structure
        // is ever wanted, derive it from `metadata` later.
        PipelineKey { _private: () }
    }
}

impl Asset for MaterialTemplate {
    type Metadata = MaterialTemplateMetadata;

    fn metadata(&self) -> &Self::Metadata {
        &self.metadata
    }
}

/// A key identifying a GPU pipeline. Opaque by design — its only producer is
/// [`MaterialTemplate::pipeline_key`] and its only consumer will be the
/// `PipelineCache` (Step 8). Internally it is backed by the template handle.
//
// Note: we keep this as a distinct type (rather than `Handle<MaterialTemplate>`)
// so the pipeline cache API can evolve independently of the asset handle.
// Step 8 will store `Handle<MaterialTemplate>` inside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PipelineKey {
    _private: (),
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
    pub fragment_shader: Handle<Shader>,
    pub vertex_buffer_layout: VertexBufferLayout,
    pub blend_state: BlendMode,
    pub depth_stencil: Option<DepthStencilConfig>,
    pub topology: Topology,
    /// Declares the uniform bindings the template's shaders expect. Drives
    /// bind group layout creation (Step 8).
    pub uniform_layout: Vec<UniformBinding>,
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
//  Uniforms — declaration (in metadata) and values (in Material instances).
// ──────────────────────────────────────────────────────────────────────────

/// Declares a single uniform binding as the template's shaders expect it.
/// Drives bind group layout creation (Step 8).
#[derive(Clone, Debug)]
pub struct UniformBinding {
    pub name: String,
    pub ty: UniformType,
    /// Bind group index + binding slot within that group. For V1 we use a
    /// single bind group (group 0), so this is the binding slot within group 0.
    pub binding_slot: u32,
    pub visibility: ShaderStage,
}

/// The type of a uniform value. Grows as new shaders need more types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UniformType {
    Mat4,
    Vec4,
    F32,
}

impl UniformType {
    /// Size in bytes of a value of this type — used to size uniform buffers.
    pub fn size(&self) -> u64 {
        match self {
            UniformType::Mat4 => 64,
            UniformType::Vec4 => 16,
            UniformType::F32 => 4,
        }
    }
}

/// A runtime uniform value. Set on a `Material` instance via
/// [`Material::set_uniform`]. Kept as a typed enum so the material can pack
/// values into a uniform buffer without the caller worrying about layout.
#[derive(Clone, Copy, Debug)]
pub enum UniformValue {
    Mat4(Mat4),
    Vec4(Vec4),
    F32(f32),
}

impl UniformValue {
    pub fn ty(&self) -> UniformType {
        match self {
            UniformValue::Mat4(_) => UniformType::Mat4,
            UniformValue::Vec4(_) => UniformType::Vec4,
            UniformValue::F32(_) => UniformType::F32,
        }
    }

    /// Writes the value into `bytes` at `offset` using WGSL's std140 layout.
    pub fn write_bytes(&self, bytes: &mut [u8], offset: usize) {
        match self {
            UniformValue::Mat4(m) => {
                let cols = m.to_cols_array();
                bytes[offset..offset + 64].copy_from_slice(bytemuck::cast_slice(&cols));
            }
            UniformValue::Vec4(v) => {
                let arr = v.to_array();
                bytes[offset..offset + 16].copy_from_slice(bytemuck::cast_slice(&arr));
            }
            UniformValue::F32(x) => {
                bytes[offset..offset + 4].copy_from_slice(bytemuck::cast_slice(&[*x]));
            }
        }
    }
}

/// Which shader stage(s) a binding is visible from. Engine-native mirror of
/// `wgpu::ShaderStages`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Both,
}

impl From<ShaderStage> for wgpu::ShaderStages {
    fn from(s: ShaderStage) -> Self {
        match s {
            ShaderStage::Vertex => wgpu::ShaderStages::VERTEX,
            ShaderStage::Fragment => wgpu::ShaderStages::FRAGMENT,
            ShaderStage::Both => wgpu::ShaderStages::VERTEX_FRAGMENT,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
//  Material — a lightweight per-instance reference to a template plus the
//  per-instance uniform values and texture bindings. Many materials share
//  one template (and thus one pipeline).
// ──────────────────────────────────────────────────────────────────────────

/// A per-instance material: a template handle plus the uniform values and
/// texture bindings that vary between instances of the same template.
///
/// `dirty` tracks whether the uniform buffer / bind groups need rebuilding
/// before the next draw (Step 8's `Material::ensure_bound` consumes it).
pub struct Material {
    template: Handle<MaterialTemplate>,
    uniforms: HashMap<String, UniformValue>,
    textures: HashMap<u32, Handle<crate::graphics::texture::Texture>>,
    dirty: bool,
}

impl Material {
    /// Creates a new material instance referencing `template` with no uniforms
    /// or textures set yet.
    pub fn new(template: Handle<MaterialTemplate>) -> Self {
        Self {
            template,
            uniforms: HashMap::new(),
            textures: HashMap::new(),
            dirty: true,
        }
    }

    pub fn template(&self) -> Handle<MaterialTemplate> {
        self.template
    }

    /// Sets a named uniform value. Marks the material `dirty` so Step 8's
    /// `ensure_bound` rebuilds the uniform buffer before the next draw.
    pub fn set_uniform(&mut self, name: impl Into<String>, value: UniformValue) {
        self.uniforms.insert(name.into(), value);
        self.dirty = true;
    }

    /// Sets the texture bound at `binding` (the bind group slot). Marks the
    /// material `dirty` so Step 8's `ensure_bound` rebuilds the bind group.
    pub fn set_texture(&mut self, binding: u32, texture: Handle<crate::graphics::texture::Texture>) {
        self.textures.insert(binding, texture);
        self.dirty = true;
    }

    pub fn uniform(&self, name: &str) -> Option<&UniformValue> {
        self.uniforms.get(name)
    }

    pub fn texture(&self, binding: u32) -> Option<Handle<crate::graphics::texture::Texture>> {
        self.textures.get(&binding).copied()
    }

    pub fn uniforms(&self) -> &HashMap<String, UniformValue> {
        &self.uniforms
    }

    pub fn textures(&self) -> &HashMap<u32, Handle<crate::graphics::texture::Texture>> {
        &self.textures
    }

    /// Whether the material's GPU resources (uniform buffer / bind groups)
    /// need rebuilding before the next draw. Consumed by Step 8's
    /// `Material::ensure_bound`.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clears the dirty flag (called by Step 8's `ensure_bound` after it has
    /// rebuilt the GPU resources).
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
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

