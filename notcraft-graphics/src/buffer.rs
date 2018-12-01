use super::error::GlResult;
use crate::context::Context;
use gl::{self, types::*};
use std::marker::PhantomData;

#[derive(Debug, Eq, PartialEq)]
pub struct RawBuffer {
    pub(crate) id: u32,
    pub(crate) len: usize,
}

impl RawBuffer {
    pub(crate) fn new(_ctx: &Context) -> Self {
        let mut id = 0;
        gl_call!(assert CreateBuffers(1, &mut id));

        RawBuffer { id, len: 0 }
    }
}

impl Drop for RawBuffer {
    fn drop(&mut self) {
        gl_call!(debug DeleteBuffers(1, &self.id));
    }
}

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
/// Usage type for buffers, provided as a performance hint. These values do not
/// affect the behavior of the buffer.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(u32)]
pub enum UsageType {
    /// The data store contents will be specified once by the application, and
    /// sourced at most a few times.
    StreamDraw = gl::STREAM_DRAW,
    /// The data store contents will be specified once by reading data from the
    /// GL, and queried at most a few times by the application.
    StreamRead = gl::STREAM_READ,
    /// The data store contents will be specified once by reading data from the
    /// GL, and sourced at most a few times
    StreamCopy = gl::STREAM_COPY,
    /// The data store contents will be specified once by the application, and
    /// sourced many times.
    StaticDraw = gl::STATIC_DRAW,
    /// The data store contents will be specified once by reading data from the
    /// GL, and queried many times by the application.
    StaticRead = gl::STATIC_READ,
    /// The data store contents will be specified once by reading data from the
    /// GL, and sourced many times.
    StaticCopy = gl::STATIC_COPY,
    /// The data store contents will be respecified repeatedly by the
    /// application, and sourced many times.
    DynamicDraw = gl::DYNAMIC_DRAW,
    /// The data store contents will be respecified repeatedly by reading data
    /// from the GL, and queried many times by the application.
    DynamicRead = gl::DYNAMIC_READ,
    /// The data store contents will be respecified repeatedly by reading data
    /// from the GL, and sourced many times.
    DynamicCopy = gl::DYNAMIC_COPY,
}

/// A handle to GPU-allocated memory. The type itself is shareable, but any
/// useful operation must be performed on
/// the current opengl thread. This is enforced by every gl-stae-interacting
/// function needing a reference to the thread-local context.
#[derive(Debug, Eq, PartialEq)]
pub struct Buffer<T> {
    pub(crate) raw: RawBuffer,
    _phantom: PhantomData<*const T>,
}

impl<T: Copy> Buffer<T> {
    pub fn new(ctx: &Context) -> Self {
        Buffer {
            raw: RawBuffer::new(ctx),
            _phantom: PhantomData,
        }
    }

    pub(crate) fn bind(&self, target: BufferTarget) {
        gl_call!(debug BindBuffer(target as u32, self.raw.id));
    }

    /// Copies data from `data` to the gpu's memory
    pub fn upload(&mut self, data: &[T], usage_type: UsageType) -> GlResult<()> {
        self.raw.len = data.len();
        // Could fail if OOM
        gl_call!(NamedBufferData(
            self.raw.id,
            (::std::mem::size_of::<T>() * data.len()) as isize,
            data.as_ptr() as *const _,
            usage_type as GLenum
        ))
    }

    pub fn len(&self) -> usize {
        self.raw.len
    }
}

pub struct BufferBuilder<'d, T> {
    usage: UsageType,
    data: Option<&'d [T]>,
}

impl<'d, T: Copy> BufferBuilder<'d, T> {
    pub fn new() -> Self {
        BufferBuilder {
            usage: UsageType::StaticDraw,
            data: None,
        }
    }

    pub fn with_usage(mut self, usage: UsageType) -> Self {
        self.usage = usage;
        self
    }

    pub fn with_data(mut self, data: &'d [T]) -> Self {
        self.data = Some(data);
        self
    }

    pub fn build(self, ctx: &Context) -> GlResult<Buffer<T>> {
        let mut buf = Buffer::new(ctx);
        if let Some(data) = self.data {
            buf.upload(data, self.usage)?;
        }
        Ok(buf)
    }
}
