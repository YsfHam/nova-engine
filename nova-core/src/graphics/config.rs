#[derive(Clone, Copy)]
pub struct GraphicsConfiguration {
    pub power_preference: wgpu::PowerPreference,
    pub present_mode: wgpu::PresentMode,
    pub alpha_mode: wgpu::CompositeAlphaMode,
}

impl GraphicsConfiguration {
    pub fn default() -> Self {
        Self {
            power_preference: wgpu::PowerPreference::HighPerformance,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
        }
    }

    pub fn with_power_preference(mut self, power_preference: wgpu::PowerPreference) -> Self {
        self.power_preference = power_preference;
        self
    }

    pub fn with_present_mode(mut self, present_mode: wgpu::PresentMode) -> Self {
        self.present_mode = present_mode;
        self
    }

    pub fn with_alpha_mode(mut self, alpha_mode: wgpu::CompositeAlphaMode) -> Self {
        self.alpha_mode = alpha_mode;
        self
    }    
}