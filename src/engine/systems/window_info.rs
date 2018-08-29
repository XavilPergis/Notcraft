use glfw::Window;
use std::sync::{Arc, Mutex};
use specs::prelude::*;
use engine::resources::*;

pub struct ViewportUpdater {
    window: Arc<Mutex<Window>>,
    prev_size: (i32, i32),
}

impl ViewportUpdater {
    pub fn new(window: &Arc<Mutex<Window>>) -> Self {
        ViewportUpdater {
            window: window.clone(),
            prev_size: (0, 0),
        }
    }
}

impl<'a> System<'a> for ViewportUpdater {
    type SystemData = Write<'a, FramebufferSize>;

    fn run(&mut self, mut size: Self::SystemData) {
        let window = self.window.lock().unwrap();
        if window.get_framebuffer_size() != self.prev_size {
            let (width, height) = window.get_framebuffer_size();
            *size = FramebufferSize {
                x: width as f64,
                y: height as f64,
            };
            unsafe {
                gl_call!(Viewport(0, 0, width, height)).expect("Failed to set viewport size");
            }
        }
    }
}
