use crate::{
    buffer::{Buffer, BufferTarget},
    context::Context,
    layout::{AttributeFormat, DataSource, InterleavedAttribute},
};
use std::{marker::PhantomData, mem};

pub struct Binder<'vao> {
    vao: &'vao mut VertexArray,
    index: usize,
}

// void glBindVertexBuffer(	GLuint bindingindex,
//  	GLuint buffer,
//  	GLintptr offset,
//  	GLsizei stride);

impl<'vao> Binder<'vao> {
    pub(crate) fn bind_buffer<T>(&mut self, buffer: &Buffer<T>) {
        gl_call!(assert BindVertexBuffer(self.index as u32, buffer.raw.id, 0, mem::size_of::<T>() as i32));

        self.index += 1;
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct VertexArray {
    pub(crate) id: u32,
    attrib: usize,
    binding: usize,
}

impl VertexArray {
    pub(crate) fn for_data_source<'v, V: DataSource<'v>>(ctx: &Context) -> Self {
        let mut vao = Self::new(ctx);
        V::setup_sources(&mut vao);
        vao
    }

    pub(crate) fn new(_ctx: &Context) -> Self {
        let mut id = 0;
        gl_call!(assert CreateVertexArrays(1, &mut id));

        VertexArray {
            id,
            attrib: 0,
            binding: 0,
        }
    }

    pub(crate) fn bind(&self) {
        gl_call!(debug BindVertexArray(self.id));
    }

    /// Pushes one buffer's worth of attributes. That is, (A, B) will push two
    /// attributes but set the binding to the current binding index
    pub(crate) fn push_binding<A: InterleavedAttribute>(&mut self) {
        A::attribute_format(|attr| {
            println!(
                "Set up attribute {} on binding {}: {:?}",
                self.attrib, self.binding, attr
            );
            self.bind();

            if attr.is_integer {
                gl_call!(assert VertexAttribIFormat(self.attrib as u32, attr.dim as i32, attr.ty as u32, attr.offset as u32));
            } else {
                gl_call!(assert VertexAttribFormat(self.attrib as u32, attr.dim as i32, attr.ty as u32, attr.normalized as u8, attr.offset as u32));
            }

            gl_call!(assert VertexAttribBinding(self.attrib as u32, self.binding as u32));
            gl_call!(assert EnableVertexAttribArray(self.attrib as u32));

            self.attrib += 1;
        });

        self.binding += 1;
    }

    pub(crate) fn binder(&mut self) -> Binder<'_> {
        Binder {
            vao: self,
            index: 0,
        }
    }
}

impl Drop for VertexArray {
    fn drop(&mut self) {
        gl_call!(debug DeleteVertexArrays(1, &self.id));
    }
}
