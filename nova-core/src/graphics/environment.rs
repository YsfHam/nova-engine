use crate::graphics::{shader::ShaderStage, uniform::UniformValue};

pub struct EnvironmentUniform {
    pub binding_slot: u32,
    pub visibilty: ShaderStage,
    pub uniform: UniformValue,
}
pub struct EnvironmentDescriptor {
    uniforms: Vec<EnvironmentUniform>,
}

impl EnvironmentDescriptor {
    pub fn new() -> Self {
        Self {
            uniforms: Vec::new(),
        }
    }

    pub fn add_uniform(mut self, uniform: EnvironmentUniform) -> Self {
        self.uniforms.push(uniform);
        self
    }

    pub fn uniforms(&self) -> &Vec<EnvironmentUniform> {
        &self.uniforms
    }
}