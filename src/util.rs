use std::sync::Mutex;
use gl_api::shader::program::LinkedProgram;
use gl_api::buffer::{UsageType, VertexBuffer};
use gl_api::vertex_array::VertexArray;
use cgmath::{Point3, Vector3};
use collision::Aabb3;

use std::cmp::Ordering;

pub fn min<S: PartialOrd + Copy>(lhs: S, rhs: S) -> S {
    match lhs.partial_cmp(&rhs) {
        Some(Ordering::Less) | Some(Ordering::Equal) | None => lhs,
        _ => rhs,
    }
}

pub fn max<S: PartialOrd + Copy>(lhs: S, rhs: S) -> S {
    match lhs.partial_cmp(&rhs) {
        Some(Ordering::Greater) | Some(Ordering::Equal) | None => lhs,
        _ => rhs,
    }
}

pub fn clamp<T: PartialOrd + Copy>(x: T, a: T, b: T) -> T {
    if x < a { a }
    else if x > b { b }
    else { x }
}

/// x / y, round towards negative inf
pub fn floor_div(x: i32, y: i32) -> i32 {
    let result = x / y;
    let remainder = x % y;
    if remainder < 0 { result - 1 } else { result }
}

fn box_verts(bbox: Aabb3<f32>, verts: &mut Vec<Vector3<f32>>) {
    let Aabb3 { min: l, max: h } = bbox;
    verts.extend(&[
        // bottom
        Vector3::new(l.x, l.y, l.z), Vector3::new(h.x, l.y, l.z), Vector3::new(l.x, l.y, h.z),
        Vector3::new(h.x, l.y, h.z), Vector3::new(l.x, l.y, h.z), Vector3::new(h.x, l.y, l.z),
        // top
        Vector3::new(l.x, h.y, l.z), Vector3::new(h.x, h.y, l.z), Vector3::new(l.x, h.y, h.z),
        Vector3::new(h.x, h.y, h.z), Vector3::new(l.x, h.y, h.z), Vector3::new(h.x, h.y, l.z),
        // back
        Vector3::new(l.x, h.y, l.z), Vector3::new(h.x, h.y, l.z), Vector3::new(l.x, l.y, l.z),
        Vector3::new(h.x, l.y, l.z), Vector3::new(l.x, l.y, l.z), Vector3::new(h.x, h.y, l.z),
        // front
        Vector3::new(l.x, h.y, h.z), Vector3::new(h.x, h.y, h.z), Vector3::new(l.x, l.y, h.z),
        Vector3::new(h.x, l.y, h.z), Vector3::new(l.x, l.y, h.z), Vector3::new(h.x, h.y, h.z),
        // left
        Vector3::new(l.x, h.y, l.z), Vector3::new(l.x, h.y, h.z), Vector3::new(l.x, l.y, l.z),
        Vector3::new(l.x, l.y, h.z), Vector3::new(l.x, l.y, l.z), Vector3::new(l.x, h.y, h.z),
        // right
        Vector3::new(h.x, h.y, l.z), Vector3::new(h.x, h.y, h.z), Vector3::new(h.x, l.y, l.z),
        Vector3::new(h.x, l.y, h.z), Vector3::new(h.x, l.y, l.z), Vector3::new(h.x, h.y, h.z),
    ][..]);
}

pub fn to_vector<S>(point: Point3<S>) -> Vector3<S> {
    Vector3::new(point.x, point.y, point.z)
}

pub fn to_point<S>(vec: Vector3<S>) -> Point3<S> {
    Point3::new(vec.x, vec.y, vec.z)
}

fn box_around_segment(a: Vector3<f32>, b: Vector3<f32>, thickness: f32) -> Aabb3<f32> {
    let min = Vector3::new(min(a.x, b.x), min(a.y, b.y), min(a.z, b.z));
    let max = Vector3::new(max(a.x, b.x), max(a.y, b.y), max(a.z, b.z));

    let tv = Vector3::new(thickness, thickness, thickness);

    Aabb3::new(to_point(min - tv), to_point(max + tv))
}

pub fn draw_frame(program: &mut LinkedProgram, bbox: Aabb3<f32>, color: Vector3<f32>, thickness: f32) {
    let Aabb3 { min: l, max: h } = bbox;
    // Front edges
    let fl = box_around_segment(Vector3::new(l.x, l.y, h.z), Vector3::new(l.x, h.y, h.z), thickness); // left
    let fr = box_around_segment(Vector3::new(h.x, l.y, h.z), Vector3::new(h.x, h.y, h.z), thickness); // right
    let ft = box_around_segment(Vector3::new(l.x, h.y, h.z), Vector3::new(h.x, h.y, h.z), thickness); // top
    let fb = box_around_segment(Vector3::new(l.x, l.y, h.z), Vector3::new(h.x, l.y, h.z), thickness); // bottom
    // Back edges
    let bl = box_around_segment(Vector3::new(l.x, l.y, l.z), Vector3::new(l.x, h.y, l.z), thickness); // left
    let br = box_around_segment(Vector3::new(h.x, l.y, l.z), Vector3::new(h.x, h.y, l.z), thickness); // right
    let bt = box_around_segment(Vector3::new(l.x, h.y, l.z), Vector3::new(h.x, h.y, l.z), thickness); // top
    let bb = box_around_segment(Vector3::new(l.x, l.y, l.z), Vector3::new(h.x, l.y, l.z), thickness); // bottom
    // Middle edges
    let tl  = box_around_segment(Vector3::new(l.x, h.y, h.z), Vector3::new(l.x, h.y, l.z), thickness); // top left
    let btl = box_around_segment(Vector3::new(l.x, l.y, h.z), Vector3::new(l.x, l.y, l.z), thickness); // bottom left
    let tr  = box_around_segment(Vector3::new(h.x, h.y, h.z), Vector3::new(h.x, h.y, l.z), thickness); // top right
    let btr = box_around_segment(Vector3::new(h.x, l.y, h.z), Vector3::new(h.x, l.y, l.z), thickness); // bottom right

    let mut buf = Vec::new();
    for &item in &[fl, fr, ft, fb, bl, br, bt, bb, tl, btl, tr, btr] {
        box_verts(item, &mut buf);
    }

    struct InternalBuffer(VertexBuffer<Vector3<f32>>);
    unsafe impl Send for InternalBuffer {}
    unsafe impl Sync for InternalBuffer {}
    struct InternalVao(VertexArray);
    unsafe impl Send for InternalVao {}
    unsafe impl Sync for InternalVao {}

    lazy_static! {
        static ref DEBUG_VBO: Mutex<InternalBuffer> = Mutex::new(InternalBuffer(VertexBuffer::new()));
        static ref DEBUG_VAO: Mutex<InternalVao> = {
            let mut vao = VertexArray::new();
            // TODO: unwrap
            vao.add_buffer(&DEBUG_VBO.lock().unwrap().0).unwrap();
            Mutex::new(InternalVao(vao))
        };
    }

    use gl;
    unsafe {
        gl::Disable(gl::CULL_FACE);
    }

    program.bind();
    {
        DEBUG_VAO.lock().unwrap().0.bind();
        let vbo = &mut DEBUG_VBO.lock().unwrap().0;
        vbo.upload(&buf, UsageType::StaticDraw).unwrap();
        vbo.bind();
        program.set_uniform("u_color", &color);
    }

    unsafe {
        gl_call!(DrawArrays(gl::TRIANGLES, 0, buf.len() as i32)).unwrap();
        gl::Enable(gl::CULL_FACE);
    }
}

