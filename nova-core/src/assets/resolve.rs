use std::collections::HashMap;

use crate::{assets::{Asset, AssetsManager, handle::Handle}, graphics::{material::{Material, MaterialTemplate}, sampler::Sampler, shader::{FragmentShader, ShaderTypeMismatch, VertexShader}, texture::Texture}};

#[derive(Debug)]
pub enum ResolutionError {
    UnresolvedDependecy,
    ShaderTypeMismatch(ShaderTypeMismatch),
}

impl From<ShaderTypeMismatch> for ResolutionError {
    fn from(value: ShaderTypeMismatch) -> Self {
        Self::ShaderTypeMismatch(value)
    }
}

type ResolutionResult<A> = Result<A, ResolutionError>;

fn resolve_asset<A: Asset>(handle: Handle<A>, assets_manager: &AssetsManager) -> ResolutionResult<&A> {
    assets_manager.get_asset(handle).ok_or(ResolutionError::UnresolvedDependecy)
}

#[derive(Clone, Copy)]
pub struct ResolvedTexture<'a> {
    pub sampler: &'a Sampler,
    pub texture: &'a Texture,
}

impl<'a> ResolvedTexture<'a> {
    pub fn new(handle: Handle<Texture>, assets_manager: &'a AssetsManager) -> ResolutionResult<Self> {
        let texture = resolve_asset(handle, assets_manager)?;
        let sampler = resolve_asset(texture.sampler_handle(), assets_manager)?;

        Ok(Self {
            texture,
            sampler
        })
    }
}

#[derive(Clone, Copy)]
pub struct ResolvedMaterialTemplate<'a> {
    pub handle: Handle<MaterialTemplate>,
    pub vertex_shader: VertexShader<'a>,
    pub fragment_shader: Option<FragmentShader<'a>>,
    pub material_template: &'a MaterialTemplate,
}

impl<'a> ResolvedMaterialTemplate<'a> {
    pub fn new(handle: Handle<MaterialTemplate>, assets_manager: &'a AssetsManager) -> ResolutionResult<Self> {
        let template = resolve_asset(handle, assets_manager)?;
        let vertex_shader = resolve_asset(template.vertex_shader(), assets_manager)?;
        let fragment_shader = match template.fragment_shader() {
            Some(fs) => Some(resolve_asset(fs, assets_manager)?.try_into()?),
            None => None
        };

        Ok(Self {
            handle,
            vertex_shader: vertex_shader.try_into()?,
            fragment_shader,
            material_template: template,
        })
    }
}

#[derive(Clone)]
pub struct ResolvedMaterial<'a> {
    pub handle: Handle<Material>,
    pub material: &'a Material,
    pub material_template: ResolvedMaterialTemplate<'a>,
    pub textures: HashMap<u32, ResolvedTexture<'a>>
}

impl<'a> ResolvedMaterial<'a> {
    pub fn new(handle: Handle<Material>, assets_manager: &'a AssetsManager) ->ResolutionResult<Self> {
        let material = resolve_asset(handle, assets_manager)?;
        let material_template =
            ResolvedMaterialTemplate::new(material.template(), assets_manager)?;
        let mut textures = HashMap::new();
        for (key, tex_handle) in material.textures() {
            textures.insert(*key, ResolvedTexture::new(*tex_handle, assets_manager)?);
        }

        Ok(Self {
            handle,
            material,
            material_template,
            textures
        })
    }
} 