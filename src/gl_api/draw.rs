use gl::{self, types::*};

pub trait BufferIndex: Copy + Sized {
    const INDEX_TYPE: GLenum;
    // HACK: Vec needs a usize to be able to index into it, so we need a mechanism to convert the mesh index into a usize
    // There might be other, better, ways to do this, but idk
    fn as_usize(self) -> usize;
}

impl BufferIndex for u8 {
    const INDEX_TYPE: GLenum = gl::UNSIGNED_BYTE;
    #[inline(always)]
    fn as_usize(self) -> usize {
        self as usize
    }
}

impl BufferIndex for u16 {
    const INDEX_TYPE: GLenum = gl::UNSIGNED_SHORT;
    #[inline(always)]
    fn as_usize(self) -> usize {
        self as usize
    }
}

impl BufferIndex for u32 {
    const INDEX_TYPE: GLenum = gl::UNSIGNED_INT;
    #[inline(always)]
    fn as_usize(self) -> usize {
        self as usize
    }
}

#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum PrimitiveType {
    Points = gl::POINTS,
    Lines = gl::LINES,
    LineStrip = gl::LINE_STRIP,
    LineLoop = gl::LINE_LOOP,
    Triangles = gl::TRIANGLES,
    TriangleStrip = gl::TRIANGLE_STRIP,
    TriangleFan = gl::TRIANGLE_FAN,
}
