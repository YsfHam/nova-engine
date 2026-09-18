use std::{any::TypeId, collections::HashMap};

use crate::{
    assets::AssetsManager, graphics::{
        material::{BindGroup, BindGroupLayout, MaterialTemplate}, shader::Shader,
    },
};

// ──────────────────────────────────────────────────────────────────────────
//  MaterialRegistry — TypeId-keyed registry of material types.
//
//  Each registered material type provides:
//  - Its `MaterialTemplate` (returned by `Material::template()`).
//  - A `template_hash` (for pipeline cache keys).
//  - A type-erased `resolve` function that, given a `GenericHandle` + the
//    `AssetsManager`, returns `&dyn AsBindGroup` (by downcasting the asset).
//
//  The registry is populated by calling `register::<M>()` for each material
//  type the engine knows about.
// ──────────────────────────────────────────────────────────────────────────

pub(crate) struct MaterialRegistration {
    pub template: MaterialTemplate,
    /// Type-erased adapter: given a GenericHandle + &AssetsManager, returns
    /// &dyn AsBindGroup (borrows the asset from the manager).
    pub resolve: fn(crate::assets::handle::WeakGenericHandle, &AssetsManager) -> Option<BindGroup>,
}

pub(crate) struct MaterialRegistry {
    registrations: HashMap<TypeId, MaterialRegistration>,
}

impl MaterialRegistry {
    pub(crate) fn new() -> Self {
        Self {
            registrations: HashMap::new(),
        }
    }

    pub(crate) fn register<M: crate::graphics::material::Material>(&mut self) {
        let template = M::template();
        self.registrations.insert(
            TypeId::of::<M>(),
            MaterialRegistration {
                template,
                resolve: |handle, assets| {
                    let any = assets.get_asset_any(handle);
                    any.downcast_ref::<M>()
                        .map(|m| m as &dyn crate::graphics::material::AsBindGroup)
                        .map(|bg| bg.as_bind_group())
                },
            },
        );
    }

    pub(crate) fn get(&self, type_id: &TypeId) -> Option<&MaterialRegistration> {
        self.registrations.get(type_id)
    }
}

// ──────────────────────────────────────────────────────────────────────────
//  Pipeline cache — keyed by template_hash + target_format + layout hashes.
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PipelineCacheKey {
    template: MaterialTemplate,
    target_format: wgpu::TextureFormat,
    sample_count: u32,
    scene_layout: BindGroupLayout,
    material_layout: BindGroupLayout,
}

/// Bundles all inputs needed to compile (or look up) a pipeline, so
/// `get_or_compile` takes a single argument instead of nine.
pub(crate) struct PipelineCompileRequest<'a> {
    pub device: &'a wgpu::Device,
    pub shader: &'a Shader,
    pub template: &'a MaterialTemplate,
    pub scene_layout: &'a wgpu::BindGroupLayout,
    pub material_layout: Option<&'a wgpu::BindGroupLayout>,
    pub target_format: wgpu::TextureFormat,
    pub sample_count: u32,
    pub scene_layout_native: BindGroupLayout,
    pub material_layout_native: BindGroupLayout,
}

pub struct Pipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: Option<wgpu::BindGroupLayout>,
}

pub struct PipelineCache {
    cache: HashMap<PipelineCacheKey, Pipeline>,
}

impl PipelineCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Returns the existing pipeline for this key, or compiles a new one.
    pub(crate) fn get_or_compile(&mut self, req: PipelineCompileRequest<'_>) -> &Pipeline {
        let key = PipelineCacheKey {
            template: req.template.clone(),
            target_format: req.target_format,
            sample_count: req.sample_count,
            scene_layout: req.scene_layout_native.clone(),
            material_layout: req.material_layout_native.clone(),
        };

        self.cache
            .entry(key)
            .or_insert_with(|| {
                Self::compile_pipeline(
                    req.device,
                    req.shader,
                    req.template,
                    req.scene_layout,
                    req.material_layout,
                    req.target_format,
                    req.sample_count,
                )
            })
    }

    fn compile_pipeline(
        device: &wgpu::Device,
        shader: &Shader,
        template: &MaterialTemplate,
        scene_layout: &wgpu::BindGroupLayout,
        material_layout: Option<&wgpu::BindGroupLayout>,
        target_format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> Pipeline {
        let vertex_attrs = template.buffer_layout.wgpu_attributes();
        let instance_attrs = template.instance_layout.as_ref().map(|l| l.wgpu_attributes());

        let vertex_wbl = if !template.buffer_layout.is_empty() {
            Some(wgpu::VertexBufferLayout {
                array_stride: template.buffer_layout.stride(),
                step_mode: template.buffer_layout.step_mode().into(),
                attributes: &vertex_attrs,
            })
        } else { None };

        let instance_wbl = if let Some(ref attrs) = instance_attrs {
            let il = template.instance_layout.as_ref().unwrap();
            Some(wgpu::VertexBufferLayout {
                array_stride: il.stride(),
                step_mode: il.step_mode().into(),
                attributes: attrs,
            })
        } else { None };

        let buffers: Vec<Option<wgpu::VertexBufferLayout>> = match (&vertex_wbl, &instance_wbl) {
            (Some(v), Some(i)) => vec![Some(v.clone()), Some(i.clone())],
            (Some(v), None) => vec![Some(v.clone())],
            (None, Some(i)) => vec![None, Some(i.clone())],
            (None, None) => vec![],
        };

        let vertex_shader = crate::graphics::shader::VertexShader::try_from(shader)
            .expect("Material template requires a vertex shader but the shader module has no vertex entry point");
        let fragment_shader = crate::graphics::shader::FragmentShader::try_from(shader).ok();

        let vertex = wgpu::VertexState {
            module: vertex_shader.module(),
            entry_point: Some(vertex_shader.entry_point()),
            buffers: &buffers,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        };

        let fragment = match &fragment_shader {
            Some(fs) => Some(wgpu::FragmentState {
                module: fs.module(),
                entry_point: Some(fs.entry_point()),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: template.blend_state.into(),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            None => None,
        };

        let bind_group_layouts: Vec<Option<&wgpu::BindGroupLayout>> = match material_layout {
            Some(ml) => vec![Some(scene_layout), Some(ml)],
            None => vec![Some(scene_layout), None],
        };

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render pipeline layout"),
            bind_group_layouts: &bind_group_layouts,
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex,
            fragment,
            primitive: wgpu::PrimitiveState {
                topology: template.topology.into(),
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: template.depth_stencil.as_ref().map(Into::into),
            multisample: wgpu::MultisampleState {
                count: sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        Pipeline {
            pipeline,
            bind_group_layout: material_layout.map(|l| l.clone()),
        }
    }
}