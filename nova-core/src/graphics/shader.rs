use std::{fs::read_to_string, path::PathBuf};

use crate::graphics::shader::ShaderEntryPoint::Both;

/// A compiled GPU shader module. Renderer-owned, cached by source hash.
pub(crate) struct Shader {
    module: wgpu::ShaderModule,
    entry_point: ShaderEntryPoint,
}

impl Shader {
    /// Creates a `Shader` by compiling WGSL source into a `wgpu::ShaderModule`.
    pub(crate) fn new(
        device: &wgpu::Device,
        label: &str,
        source_str: &str,
        entry_point: ShaderEntryPoint,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(source_str.into()),
        });
        Self { module, entry_point }
    }

    pub(crate) fn module(&self) -> &wgpu::ShaderModule {
        &self.module
    }
}

/// Loads shader source text from a [`ShaderSource`]: reads the file or
/// returns the inline string.
pub(crate) fn load_source(source: &ShaderSource) -> Result<String, std::io::Error> {
    match source {
        ShaderSource::File(path) => read_to_string(path),
        ShaderSource::Inline(src) => Ok(src.clone()),
    }
}

/// Derives a label for a shader from its source (file name or "inline").
pub(crate) fn source_label(source: &ShaderSource) -> String {
    match source {
        ShaderSource::File(path) => path
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| "Shader".to_string()),
        ShaderSource::Inline(_) => "Inline shader".to_string(),
    }
}

#[derive(Debug)]
pub struct ShaderTypeMismatch {
    pub expected: ShaderStage,
    pub found: ShaderStage,
}

/// Which shader stage(s) a binding is visible from. Engine-native mirror of
/// `wgpu::ShaderStages`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

/// The WGSL entry point(s) a shader module exposes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ShaderEntryPoint {
    Vertex(String),
    Fragment(String),
    Both {
        vs_entry_point: String,
        fs_entry_point: String,
    },
}

/// A borrowed reference to the vertex stage of a compiled [`Shader`].
#[derive(Clone, Copy)]
pub struct VertexShader<'a> {
    inner: &'a Shader,
    entry_point: &'a str,
}

impl<'a> VertexShader<'a> {
    pub fn entry_point(&self) -> &'a str {
        self.entry_point
    }

    pub fn module(&self) -> &wgpu::ShaderModule {
        self.inner.module()
    }
}

impl<'a> TryFrom<&'a Shader> for VertexShader<'a> {
    type Error = ShaderTypeMismatch;

    fn try_from(shader: &'a Shader) -> Result<Self, Self::Error> {
        let entry_point = match &shader.entry_point {
            ShaderEntryPoint::Vertex(entry_point)
            | Both { vs_entry_point: entry_point, .. } => entry_point,
            ShaderEntryPoint::Fragment(_) => return Err(ShaderTypeMismatch {
                expected: ShaderStage::Vertex,
                found: ShaderStage::Fragment
            })
        };

        Ok(Self {
            inner: shader,
            entry_point,
        })
    }
}

/// A borrowed reference to the fragment stage of a compiled [`Shader`].
#[derive(Clone, Copy)]
pub struct FragmentShader<'a> {
    inner: &'a Shader,
    entry_point: &'a str,
}

impl<'a> FragmentShader<'a> {
    pub fn entry_point(&self) -> &'a str {
        self.entry_point
    }

    pub fn module(&self) -> &wgpu::ShaderModule {
        self.inner.module()
    }
}

impl<'a> TryFrom<&'a Shader> for FragmentShader<'a> {
    type Error = ShaderTypeMismatch;

    fn try_from(shader: &'a Shader) -> Result<Self, Self::Error> {
        let entry_point = match &shader.entry_point {
            ShaderEntryPoint::Fragment(entry_point)
            | Both { fs_entry_point: entry_point, .. } => entry_point,
            ShaderEntryPoint::Vertex(_) => return Err(ShaderTypeMismatch {
                expected: ShaderStage::Fragment,
                found: ShaderStage::Vertex,
            }),
        };

        Ok(Self {
            inner: shader,
            entry_point,
        })
    }
}

/// Where shader source comes from. `Inline` enables embedded default shaders
/// without touching the filesystem.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ShaderSource {
    File(PathBuf),
    Inline(String),
}

/// Groups a [`ShaderSource`] with its [`ShaderEntryPoint`] into a single
/// hashable unit, used as a `MaterialTemplate` field and as a shader cache key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShaderInfo {
    pub source: ShaderSource,
    pub entry_point: ShaderEntryPoint,
}