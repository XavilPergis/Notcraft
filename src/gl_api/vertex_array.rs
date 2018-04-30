use super::error::GlResult;
use std::mem;
use gl;
use gl::types::*;
use gl_api::buffer::VertexBuffer;
use gl_api::layout::{AttributeLayout, InternalLayout};

pub struct VertexArray {
    pub(crate) id: GLuint,
    index: usize,
}

impl !Send for VertexArray {}
impl !Sync for VertexArray {}

impl VertexArray {
    pub fn new() -> Self {
        let mut id = 0;
        // UNWRAP: Can only fail if count is negative
        unsafe { gl_call!(GenVertexArrays(1, &mut id)).unwrap(); }
        VertexArray { id, index: 0 }
    }

    pub fn bind(&self) {
        // UNWRAP: our ID should always be valid
        unsafe { gl_call!(BindVertexArray(self.id)).unwrap(); }
    }

    unsafe fn attrib_ipointer<T>(&self, layout: AttributeLayout) -> GlResult<()> {
        let index = self.index as u32;
        let size = mem::size_of::<T>() as i32;
        let offset = layout.attrib_offset as *const _;
        gl_call!(VertexAttribIPointer(index, layout.attrib_size, layout.attrib_type, size, offset))
    }

    unsafe fn attrib_pointer<T>(&self, layout: AttributeLayout, normalized: bool) -> GlResult<()> {
        let index = self.index as u32;
        let size = mem::size_of::<T>() as i32;
        let offset = layout.attrib_offset as *const _;
        let normalized = normalized as u8;
        gl_call!(VertexAttribPointer(index, layout.attrib_size, layout.attrib_type, normalized, size, offset))
    }

    pub fn add_buffer<T: InternalLayout>(&mut self, buffer: &VertexBuffer<T>) -> GlResult<()> {
        self.bind();
        buffer.bind();

        for element in T::layout() {
            unsafe {
                // Can fail if `self.index` is greater than `glGet(GL_MAX_VERTEX_ATTRIBS)`
                gl_call!(EnableVertexAttribArray(self.index as GLuint))?;
                // TODO: this feels like a hack
                match element.attrib_type {
                    | gl::BYTE | gl::UNSIGNED_BYTE
                    | gl::SHORT | gl::UNSIGNED_SHORT
                    | gl::INT | gl::UNSIGNED_INT
                      => self.attrib_ipointer::<T>(element)?,
                    _ => self.attrib_pointer::<T>(element, false)?,
                }
            }

            self.index += 1;
        }

        Ok(())
    }
}
