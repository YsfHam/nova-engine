#[derive(Clone, Copy)]
pub struct GraphicsConfiguration {
    pub power_preference: PowerPreference,
    pub present_mode: PresentMode,
    pub alpha_mode: CompositeAlphaMode,
}

impl GraphicsConfiguration {
    pub fn default() -> Self {
        Self {
            power_preference: PowerPreference::HighPerformance,
            present_mode: PresentMode::AutoVsync,
            alpha_mode: CompositeAlphaMode::Auto,
        }
    }

    pub fn with_power_preference(mut self, power_preference: PowerPreference) -> Self {
        self.power_preference = power_preference;
        self
    }

    pub fn with_present_mode(mut self, present_mode: PresentMode) -> Self {
        self.present_mode = present_mode;
        self
    }

    pub fn with_alpha_mode(mut self, alpha_mode: CompositeAlphaMode) -> Self {
        self.alpha_mode = alpha_mode;
        self
    }    
}

#[derive(Clone, Copy)]
pub enum PowerPreference {
    HighPerformance,
    LowPower,
    None
}

impl From<PowerPreference> for wgpu::PowerPreference {
    fn from(value: PowerPreference) -> Self {
        match value {
            PowerPreference::HighPerformance => wgpu::PowerPreference::HighPerformance,
            PowerPreference::LowPower => wgpu::PowerPreference::LowPower,
            PowerPreference::None => wgpu::PowerPreference::None,
        }
    }
}

#[derive(Clone, Copy)]
pub enum PresentMode {
    AutoVsync,
    AutoNoVsync,
    Fifo,
    FifoRelaxed,
    Immediate,
    Mailbox
}

impl From<PresentMode> for wgpu::PresentMode {
    fn from(value: PresentMode) -> Self {
        match value {
            PresentMode::AutoVsync => wgpu::PresentMode::AutoVsync,
            PresentMode::AutoNoVsync => wgpu::PresentMode::AutoNoVsync,
            PresentMode::Fifo => wgpu::PresentMode::Fifo,
            PresentMode::FifoRelaxed => wgpu::PresentMode::FifoRelaxed,
            PresentMode::Immediate => wgpu::PresentMode::Immediate,
            PresentMode::Mailbox => wgpu::PresentMode::Mailbox,
        }
    }
}

#[derive(Clone, Copy)]
pub enum CompositeAlphaMode {
    Auto,
    Opaque,
    PreMultiplied,
    PostMultiplied,
    Inherit
}

impl From<CompositeAlphaMode> for wgpu::CompositeAlphaMode {
    fn from(value: CompositeAlphaMode) -> Self {
        match value {
            CompositeAlphaMode::Auto => wgpu::CompositeAlphaMode::Auto,
            CompositeAlphaMode::Opaque => wgpu::CompositeAlphaMode::Opaque,
            CompositeAlphaMode::PreMultiplied => wgpu::CompositeAlphaMode::PreMultiplied,
            CompositeAlphaMode::PostMultiplied => wgpu::CompositeAlphaMode::PostMultiplied,
            CompositeAlphaMode::Inherit => wgpu::CompositeAlphaMode::Inherit,
        }
    }
}