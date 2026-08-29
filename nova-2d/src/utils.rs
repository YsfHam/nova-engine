#[derive(Copy, Clone)]
pub struct Rect<T> {
    pub top: T,
    pub left: T,
    pub bottom: T,
    pub right: T
}

pub type RectF32 = Rect<f32>;