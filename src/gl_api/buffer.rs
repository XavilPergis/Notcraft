use super::error::GlResult;
use std::marker::PhantomData;
use gl::types::*;
use gl;

mod sealed {
    pub trait Sealed {}
}

pub trait BufferTarget: sealed::Sealed {
    const TARGET: GLenum;
}

// Might allow for glBindBufferBase and whatnot in the future
pub trait IndexedTarget {}

macro_rules! buffer_target {
    ($name:ident: $enum:expr) => (
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
        pub struct $name;
        impl sealed::Sealed for $name {}
        impl BufferTarget for $name { const TARGET: GLenum = $enum; }
    );

    ($name:ident: indexed $enum:expr) => (
        buffer_target!($name: $enum);
        impl IndexedTarget for $name {}
    )
}

// Buffer targets described in section 6.1 of spec
buffer_target!(Array: gl::ARRAY_BUFFER);
buffer_target!(Element: gl::ELEMENT_ARRAY_BUFFER);
buffer_target!(Uniform: indexed gl::UNIFORM_BUFFER);
buffer_target!(PixelPack: gl::PIXEL_PACK_BUFFER);
buffer_target!(PixelUnpack: gl::PIXEL_UNPACK_BUFFER);
buffer_target!(Query: gl::QUERY_BUFFER);
buffer_target!(ShaderStorage: indexed gl::SHADER_STORAGE_BUFFER);
buffer_target!(Texture: gl::TEXTURE_BUFFER);
buffer_target!(TransformFeedback: indexed gl::TRANSFORM_FEEDBACK_BUFFER);
buffer_target!(AtomicCounter: indexed gl::ATOMIC_COUNTER_BUFFER);

// Values from section 6.2 of spec
/// Usage type for buffers, provided as a performance hint. These values do not affect the behavior
/// of the buffer.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(u32)]
pub enum UsageType {
    /// The data store contents will be specified once by the application, and sourced at most a few times.
    StreamDraw = gl::STREAM_DRAW,
    /// The data store contents will be specified once by reading data from the GL, and queried at most a few times by the application.
    StreamRead = gl::STREAM_READ,
    /// The data store contents will be specified once by reading data from the GL, and sourced at most a few times
    StreamCopy = gl::STREAM_COPY,
    /// The data store contents will be specified once by the application, and sourced many times.
    StaticDraw = gl::STATIC_DRAW,
    /// The data store contents will be specified once by reading data from the GL, and queried many times by the application.
    StaticRead = gl::STATIC_READ,
    /// The data store contents will be specified once by reading data from the GL, and sourced many times.
    StaticCopy = gl::STATIC_COPY,
    /// The data store contents will be respecified repeatedly by the application, and sourced many times.
    DynamicDraw = gl::DYNAMIC_DRAW,
    /// The data store contents will be respecified repeatedly by reading data from the GL, and queried many times by the application.
    DynamicRead = gl::DYNAMIC_READ,
    /// The data store contents will be respecified repeatedly by reading data from the GL, and sourced many times.
    DynamicCopy = gl::DYNAMIC_COPY,
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct Buffer<T, B: BufferTarget> {
    pub(crate) id: GLuint,
    length: usize,
    phantom: PhantomData<(T, B)>
}

impl<T, B: BufferTarget> !Send for Buffer<T, B> {}
impl<T, B: BufferTarget> !Sync for Buffer<T, B> {}

impl<T, B: BufferTarget> Buffer<T, B> {
    pub fn new() -> Self {
        let mut id = 0;
        // UNWRAP: Could only error if the amount is negative
        unsafe { gl_call!(GenBuffers(1, &mut id)).unwrap(); }
        Buffer { id, length: 0, phantom: PhantomData }
    }

    pub fn bind(&self) {
        // UNWRAP: Could only error if the buffer type is invalid
        unsafe { gl_call!(BindBuffer(B::TARGET, self.id)).unwrap(); }
    }

    /// Copies data from `data` to the gpu's memory
    pub fn upload(&mut self, data: &[T], usage_type: UsageType) -> GlResult<()> {
        unsafe {
            self.bind();
            self.length = data.len();
            // Could fail if OOM
            gl_call!(BufferData(B::TARGET,
                (::std::mem::size_of::<T>() * data.len()) as isize,
                data.as_ptr() as *const _,
                usage_type as GLenum))
        }
    }

    pub fn len(&self) -> usize {
        self.length
    }
}

impl<T, B: BufferTarget> Drop for Buffer<T, B> {
    fn drop(&mut self) {
        unsafe {
            // UNWRAP: can only fail if count is negative, which it isn't
            gl_call!(DeleteBuffers(1, &self.id)).unwrap();
        }
    }
}

pub type VertexBuffer<T> = Buffer<T, Array>;
pub type ElementBuffer<T> = Buffer<T, Element>;
