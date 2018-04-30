use super::error::GlResult;
use std::marker::PhantomData;
use gl::types::*;
use gl;

mod sealed {
    pub trait Sealed {}
}

pub trait BufferType: sealed::Sealed {
    const BUFFER_TYPE: GLenum;
}

macro_rules! buffer_type {
    ($name:ident: $enum:expr) => (
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
        pub struct $name;
        impl sealed::Sealed for $name {}
        impl BufferType for $name { const BUFFER_TYPE: GLenum = $enum; }
    )
}

buffer_type!(Array: gl::ARRAY_BUFFER);
buffer_type!(Element: gl::ELEMENT_ARRAY_BUFFER);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(u32)]
pub enum UsageType {
    Static = gl::STATIC_DRAW,
    Dynamic = gl::DYNAMIC_DRAW,
    Stream = gl::STREAM_DRAW,
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct Buffer<T, B: BufferType> {
    pub(crate) id: GLuint,
    length: usize,
    phantom: PhantomData<(T, B)>
}

impl<T, B: BufferType> !Send for Buffer<T, B> {}
impl<T, B: BufferType> !Sync for Buffer<T, B> {}

impl<T, B: BufferType> Buffer<T, B> {
    pub fn new() -> Self {
        let mut id = 0;
        // UNWRAP: Could only error if the amount is negative
        unsafe { gl_call!(GenBuffers(1, &mut id)).unwrap(); }
        Buffer { id, length: 0, phantom: PhantomData }
    }

    pub fn bind(&self) {
        // UNWRAP: Could only error if the buffer type is invalid
        unsafe { gl_call!(BindBuffer(B::BUFFER_TYPE, self.id)).unwrap(); }
    }

    /// Copies data from `data` to the gpu's memory
    pub fn upload(&mut self, data: &[T], usage_type: UsageType) -> GlResult<()> {
        unsafe {
            self.bind();
            self.length = data.len();
            // Could fail if OOM
            gl_call!(BufferData(B::BUFFER_TYPE,
                (::std::mem::size_of::<T>() * data.len()) as isize,
                data.as_ptr() as *const _,
                usage_type as GLenum))
        }
    }

    pub fn len(&self) -> usize {
        self.length
    }

    // pub fn map_buffer_mut<'b, 'd: 'b>(&'b mut self) -> BufferViewMut<'b, 'd, T, B> {
    //     unsafe {
    //         let data_ptr = gl_call!(MapBuffer(B::BUFFER_TYPE, gl::READ_WRITE)) as *mut T;
    //         BufferViewMut {
    //             mapped_slice: ::std::slice::from_raw_parts_mut(data_ptr, self.length),
    //             buffer: self,
    //         }
    //     }
    // }
}

// pub struct BufferViewMut<'b, 'd: 'b, T: 'd, B: BufferType + 'static> {
//     mapped_slice: &'d mut [T],
//     buffer: &'b mut Buffer<T, B>,
// }

// impl<'b, 'd, T: 'd, B: BufferType + 'static> Drop for BufferViewMut<'b, 'd, T, B> {
//     fn drop(&mut self) {
//         unsafe {
//             self.buffer.bind();
//             gl_call!(UnmapBuffer(B::BUFFER_TYPE));
//         }
//     }
// }

impl<T, B: BufferType> Drop for Buffer<T, B> {
    fn drop(&mut self) {
        unsafe {
            // UNWRAP: can only fail if count is negative, which it isn't
            gl_call!(DeleteBuffers(1, &self.id)).unwrap();
        }
    }
}

pub type VertexBuffer<T> = Buffer<T, Array>;
pub type ElementBuffer<T> = Buffer<T, Element>;
