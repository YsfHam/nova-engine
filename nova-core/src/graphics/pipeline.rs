use std::collections::HashMap;

use crate::{assets::{handle::Handle, resolve::ResolvedMaterialTemplate}, graphics::material::MaterialTemplate  };

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct PipelineCacheKey {
    material_template_handle: Handle<MaterialTemplate>,
    target_format: wgpu::TextureFormat,
}

pub struct PipelineDescriptor<'a> {
    pub material_template: ResolvedMaterialTemplate<'a>,
    pub scene_bind_group_layout: &'a wgpu::BindGroupLayout,
    pub target_format: wgpu::TextureFormat,
}


#[derive(Clone)]
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

    pub fn get_or_compile(&mut self, device: &wgpu::Device, desc: PipelineDescriptor<'_>) -> &Pipeline {

        let key = PipelineCacheKey {
            material_template_handle: desc.material_template.handle,
            target_format: desc.target_format,
        };

        self.cache.entry(key)
        .or_insert_with_key(|key| Self::compile_pipeline(device, key, desc) )
    }

    fn compile_pipeline(device: &wgpu::Device, key: &PipelineCacheKey, desc: PipelineDescriptor<'_>) -> Pipeline  {

        let bind_group_layout = Self::create_bind_group_layout(device, &desc.material_template);

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render pipeline layout"),
            bind_group_layouts: &[
                Some(desc.scene_bind_group_layout),
                bind_group_layout.as_ref()
            ],
            immediate_size: 0,
        });

        let template = desc.material_template;

        // Vertex buffer layouts: include the template's layout only when it
        // declares attributes. An empty layout (shader-generated vertices via
        // `vertex_index`) must be omitted, otherwise wgpu requires a vertex
        // buffer to be bound at draw time.
        // When an instance layout is present, it's declared as a second vertex
        // buffer at slot 1 (step mode Instance).
        let vbl = template.material_template.buffer_layout();
        let il = template.material_template.instance_layout();

        // Build the buffers slice. The vertex layout goes first; if the
        // instance layout exists and is non-empty, it's appended.
        // wgpu expects &[Option<VertexBufferLayout>] — each entry is Some
        // (declares a buffer) or None (no buffer at that slot).
        let vertex_wbl: Option<wgpu::VertexBufferLayout> = vbl.try_into().ok();
        let instance_wbl: Option<wgpu::VertexBufferLayout> = il.and_then(|l| l.try_into().ok());

        let buffers: Vec<Option<wgpu::VertexBufferLayout>> = match (&vertex_wbl, &instance_wbl) {
            (Some(v), Some(i)) => vec![Some(v.clone()), Some(i.clone())],
            (Some(v), None) => vec![Some(v.clone())],
            (None, Some(i)) => vec![None, Some(i.clone())], // slot 0 empty, slot 1 = instance
            (None, None) => vec![],
        };

        let vertex = wgpu::VertexState {
            module: template.vertex_shader.module(),
            entry_point: Some(template.vertex_shader.entry_point()),
            buffers: &buffers,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        };

        let fragment = match &template.fragment_shader {
            Some(fs) => Some(wgpu::FragmentState {
                module: fs.module(),
                entry_point: Some(fs.entry_point()),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: key.target_format,
                    blend: template.material_template.blend_state().into(),
                    write_mask: wgpu::ColorWrites::ALL,
                })]
            }),

            None => None
        };


        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex,
            fragment,
            primitive: wgpu::PrimitiveState {
                topology: template.material_template.topology().into(),
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: template.material_template.depth_stencil().map(Into::into),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        Pipeline {
            pipeline,
            bind_group_layout,
        }
    }

    fn create_bind_group_layout(device: &wgpu::Device, template: &ResolvedMaterialTemplate<'_>) -> Option<wgpu::BindGroupLayout> {
        let uniform_bindings = template.material_template.uniform_layout();

        let uniform_bindings_entries = uniform_bindings.iter()
            .map(|uniform_binding| wgpu::BindGroupLayoutEntry {
                binding: uniform_binding.binding_slot,
                visibility: uniform_binding.visibility.into(),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });

        let texture_bindings = template.material_template.texture_layout();
        let texture_bindings_entries = 
            texture_bindings
            .iter()
            .flat_map(|texture_binding| {
                [
                    wgpu::BindGroupLayoutEntry {
                        binding: texture_binding.texture_binding_slot,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: texture_binding.is_filterable() },
                            view_dimension: texture_binding.view_dimension.into(),
                            multisampled: texture_binding.multisampled,
                        },
                        count: None,
                    },

                    wgpu::BindGroupLayoutEntry {
                        binding: texture_binding.sample_binding_slot,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(texture_binding.sampler_binding_type.into()),
                        count: None,
                    }
                ]
            });


        let entries = 
            uniform_bindings_entries.chain(texture_bindings_entries) 
            .collect::<Vec<_>>();      

        if entries.is_empty() {
            return None;
        }
        
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Uniforms bind group layout"),
            entries: &entries,
        });

        Some(layout)
        
    }
}