use gl;

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(u32)]
pub enum PolygonMode {
    Point = gl::POINT,
    Line = gl::LINE,
    Fill = gl::FILL,
}

pub fn polygon_mode(mode: PolygonMode) {
    // Invalid enums are not possible here; GL_FRONT_AND_BACK is valid, and `mode` is a valid enum
    unsafe {
        gl::PolygonMode(gl::FRONT_AND_BACK, mode as u32);
    }
}

pub enum ClearMode {
    Color(f32, f32, f32, f32),
    Depth(f64),
}

pub fn clear(mode: ClearMode) {
    unsafe {
        match mode {
            ClearMode::Color(r, g, b, a) => {
                gl::Clear(gl::COLOR_BUFFER_BIT);
                gl::ClearColor(r, g, b, a);
            }
            ClearMode::Depth(n) => {
                gl::Clear(gl::DEPTH_BUFFER_BIT);
                gl::ClearDepth(n);
            } // TODO: stencil buffer
        }
    }
}
