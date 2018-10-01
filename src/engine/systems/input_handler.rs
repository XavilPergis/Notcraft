use specs::shred::PanicHandler;
use std::sync::{Arc, Mutex};
use specs::prelude::*;
use shrev::EventChannel;
use engine::components::*;
use engine::resources::*;
use glfw::{Window, WindowEvent, Key, Action};
use cgmath::Deg;

pub struct SmoothCamera;

impl<'a> System<'a> for SmoothCamera {
    type SystemData = (WriteStorage<'a, Transform>, ReadStorage<'a, LookTarget>, Read<'a, Dt>);

    fn run(&mut self, (mut transforms, targets, dt): Self::SystemData) {
        for (tfm, target) in (&mut transforms, &targets).join() {
            use util::lerp_angle;
            tfm.orientation.x = lerp_angle(tfm.orientation.x, target.x, 12.0 * dt.as_secs());
            tfm.orientation.y = lerp_angle(tfm.orientation.y, target.y, 12.0 * dt.as_secs());
        }
    }
}

pub struct InputHandler {
    window: Arc<Mutex<Window>>,
    events_handle: ReaderId<WindowEvent>,
    wireframe: bool,
    capture_mouse: bool,
}

impl InputHandler {
    pub fn new(window: &Arc<Mutex<Window>>, event_channel: &mut EventChannel<WindowEvent>) -> Self {
        InputHandler {
            window: window.clone(),
            events_handle: event_channel.register_reader(),
            wireframe: false,
            capture_mouse: false,
        }
    }
}

use gl_api::misc::*;
use glfw::CursorMode;

fn set_wireframe(wf: bool) {
    polygon_mode(if wf { PolygonMode::Line } else { PolygonMode::Fill });
}

fn set_mouse_capture(window: &mut ::glfw::Window, capture: bool) {
    window.set_cursor_mode(if capture { CursorMode::Disabled } else { CursorMode::Normal });
}

impl<'a> System<'a> for InputHandler {
    type SystemData = (
        Read<'a, EventChannel<WindowEvent>>,
        WriteStorage<'a, LookTarget>,
        WriteStorage<'a, MoveDelta>,
        Write<'a, CursorPos>,
        Write<'a, StopGameLoop>,
        Write<'a, ActiveDirections>,
        Write<'a, ViewFrustum, PanicHandler>,
    );

    fn run(&mut self, (window_events, mut look_targets, mut move_deltas, mut cursor_pos, mut stop_flag, mut active_directions, mut frustum): Self::SystemData) {
        // for delta in (&mut look_deltas).join() { *delta = LookDelta::default(); }
        for delta in (&mut move_deltas).join() { *delta = MoveDelta::default(); }

        for event in window_events.read(&mut self.events_handle) {
            match event {
                WindowEvent::Close | WindowEvent::Key(Key::Escape, _, Action::Press, _) => { stop_flag.0 = true; break; },
                // WindowEvent::Key(Key::F1, _, Action::Press, _) => modes.show_debug_frames = !modes.show_debug_frames,
                WindowEvent::Key(Key::F2, _, Action::Press, _) => { self.wireframe = !self.wireframe; set_wireframe(self.wireframe); },
                WindowEvent::Key(Key::F3, _, Action::Press, _) => { self.capture_mouse = !self.capture_mouse; set_mouse_capture(&mut *self.window.lock().unwrap(), self.capture_mouse) },
                WindowEvent::Key(Key::Z, _, Action::Press, _) => frustum.fov = Deg(20.0),
                WindowEvent::Key(Key::Z, _, Action::Release, _) => frustum.fov = Deg(80.0),
                WindowEvent::CursorPos(x, y) => {
                    // The position resource is last frame's cursor position
                    let lx = cursor_pos.x;
                    let ly = cursor_pos.y;

                    cursor_pos.x = *x;
                    cursor_pos.y = *y;

                    let dx = (x - lx) / 3.0;
                    let dy = (y - ly) / 3.0;

                    for target in (&mut look_targets).join() {
                        // Ok, I know this looks weird, but `target` describes which *axis* should be rotated around.
                        // It just so happens that the Y coordinate of the mouse corresponds to a rotation around the X axis
                        // So that's why we add the change in x to the y component of the look target. 
                        target.x = Deg(::util::clamp(target.x.0 + dy, -90.0, 90.0));
                        target.y += Deg(dx);
                    }
                },

                WindowEvent::Key(Key::W, _, Action::Press, _) => { active_directions.front = true; },
                WindowEvent::Key(Key::S, _, Action::Press, _) => { active_directions.back = true; },
                WindowEvent::Key(Key::A, _, Action::Press, _) => { active_directions.left = true; },
                WindowEvent::Key(Key::D, _, Action::Press, _) => { active_directions.right = true; },
                WindowEvent::Key(Key::Space, _, Action::Press, _) => { active_directions.up = true; },
                WindowEvent::Key(Key::LeftShift, _, Action::Press, _) => { active_directions.down = true; },

                WindowEvent::Key(Key::W, _, Action::Release, _) => { active_directions.front = false; },
                WindowEvent::Key(Key::S, _, Action::Release, _) => { active_directions.back = false; },
                WindowEvent::Key(Key::A, _, Action::Release, _) => { active_directions.left = false; },
                WindowEvent::Key(Key::D, _, Action::Release, _) => { active_directions.right = false; },
                WindowEvent::Key(Key::Space, _, Action::Release, _) => { active_directions.up = false; },
                WindowEvent::Key(Key::LeftShift, _, Action::Release, _) => { active_directions.down = false; },

                _ => {}
            }
        }


    }
}