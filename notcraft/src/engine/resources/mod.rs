use std::time::Duration;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct StopGameLoop(pub bool);

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Dt(pub Duration);

impl Dt {
    pub fn as_secs(&self) -> f32 {
        self.0.as_secs() as f32 + self.0.subsec_nanos() as f32 * 1e-9
    }
}
