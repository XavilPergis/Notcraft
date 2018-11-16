use gl_api::buffer::Buffer;
use gl_api::context::Context;
use gl_api::layout::{AttributeFormat, Layout};
use gl_api::objects::RawVertexArray;
use std::marker::PhantomData;

// NOTE: this vertex array type only supports one vertex buffer bound at binding index 0 (for now at least)
#[derive(Debug, Eq, PartialEq)]
pub struct VertexArray<V> {
    crate raw: RawVertexArray,
    attribs: Vec<AttributeFormat>,
    _marker: PhantomData<V>,
}

impl<V: Layout> VertexArray<V> {
    pub fn for_vertex_type(ctx: &Context) -> Self {
        let vao = VertexArray {
            raw: RawVertexArray::new(ctx),
            attribs: V::layout(),
            _marker: PhantomData,
        };

        // TODO: is this actually correct?
        let mut byte_offset = 0;
        for (attr, fmt) in vao.attribs.iter().enumerate() {
            vao.enable_attribute(attr);
            vao.set_attribute_format(attr, *fmt, byte_offset);
            vao.set_attribute_binding(attr, 0);
            byte_offset += fmt.size();
        }

        vao
    }

    fn enable_attribute(&self, attr: usize) {
        gl_call!(assert EnableVertexArrayAttrib(self.raw.id, attr as u32));
    }

    fn set_attribute_binding(&self, attr: usize, binding: usize) {
        gl_call!(assert VertexArrayAttribBinding(self.raw.id, attr as u32, binding as u32));
    }

    fn set_attribute_format(&self, attr: usize, fmt: AttributeFormat, offset: usize) {
        if fmt.ty.is_integer() {
            gl_call!(assert VertexArrayAttribIFormat(self.raw.id, attr as u32, fmt.dim as i32, fmt.ty as u32, offset as u32));
        } else {
            gl_call!(assert VertexArrayAttribFormat(self.raw.id, attr as u32, fmt.dim as i32, fmt.ty as u32, gl::FALSE, offset as u32));
        }
    }

    pub fn bind(&self) {
        // UNWRAP: our ID should always be valid
        gl_call!(debug BindVertexArray(self.raw.id));
    }

    pub fn set_buffer(&mut self, buffer: &Buffer<V>) {
        // all of our attributes are on binding index 0 (second param)
        // the data will start at the first element of the buffer (fourth param)
        gl_call!(assert VertexArrayVertexBuffer(self.raw.id, 0, buffer.raw.id, 0, ::std::mem::size_of::<V>() as i32));
    }

    pub fn with_buffer(mut self, buffer: &Buffer<V>) -> Self {
        self.set_buffer(buffer);
        self
    }
}
