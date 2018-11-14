use super::error::GlResult;
use gl;
use gl::types::*;
use gl_api::context::Context;
use gl_api::objects::RawBuffer;
use std::marker::PhantomData;

// Buffer targets described in section 6.1 of spec
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum BufferTarget {
    Array = gl::ARRAY_BUFFER,
    Element = gl::ELEMENT_ARRAY_BUFFER,
    Uniform = gl::UNIFORM_BUFFER,
    PixelPack = gl::PIXEL_PACK_BUFFER,
    PixelUnpack = gl::PIXEL_UNPACK_BUFFER,
    Query = gl::QUERY_BUFFER,
    ShaderStorage = gl::SHADER_STORAGE_BUFFER,
    Texture = gl::TEXTURE_BUFFER,
    TransformFeedback = gl::TRANSFORM_FEEDBACK_BUFFER,
    AtomicCounter = gl::ATOMIC_COUNTER_BUFFER,
}

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

#[derive(Debug, Eq, PartialEq)]
pub struct Buffer<T> {
    crate raw: RawBuffer,
    length: usize,
    phantom: PhantomData<*mut T>,
}

impl<T> Buffer<T> {
    pub fn new(ctx: &Context) -> Self {
        Buffer {
            raw: RawBuffer::new(ctx),
            length: 0,
            phantom: PhantomData,
        }
    }

    pub fn bind(&self, target: BufferTarget) {
        gl_call!(debug BindBuffer(target as u32, self.raw.id));
    }

    /// Copies data from `data` to the gpu's memory
    pub fn upload(&mut self, data: &[T], usage_type: UsageType) -> GlResult<()> {
        self.bind(BufferTarget::Array);
        self.length = data.len();
        // Could fail if OOM
        gl_call!(BufferData(
            BufferTarget::Array as u32,
            (::std::mem::size_of::<T>() * data.len()) as isize,
            data.as_ptr() as *const _,
            usage_type as GLenum
        ))
    }

    pub fn len(&self) -> usize {
        self.length
    }
}
