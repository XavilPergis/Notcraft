use gl;

use crate::{
    layout::DataSource, limits::Limits, program::Program, vertex_array::VertexArray, PrimitiveType,
};

use std::{
    any::TypeId,
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ViewportRect {
    pub x: i32,
    pub width: u32,
    pub y: i32,
    pub height: u32,
}

impl ViewportRect {
    fn query() -> Self {
        let mut r = [0i32; 4];
        gl_call!(assert GetIntegerv(gl::VIEWPORT, r.as_mut_ptr()));
        ViewportRect {
            x: r[0],
            y: r[1],
            width: r[2] as u32,
            height: r[3] as u32,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Context(Rc<ContextInner>);

#[derive(Debug)]
pub struct ContextInner {
    // Make sure this isn't Send or Sync
    _marker: ::std::marker::PhantomData<*mut ()>,

    pub(crate) limits: Limits,
    pub(crate) viewport: Cell<ViewportRect>,

    pub(crate) format_cache: RefCell<HashMap<TypeId, VertexArray>>,
}

impl Context {
    pub fn load<F>(mut load_fn: F) -> Context
    where
        F: FnMut(&'static str) -> *const (),
    {
        gl::load_with(|symbol| load_fn(symbol) as *const _);

        Context(Rc::new(ContextInner {
            _marker: ::std::marker::PhantomData,

            limits: Limits::load(),
            viewport: Cell::new(ViewportRect::query()),
            format_cache: RefCell::new(HashMap::new()),
        }))
    }

    // self.ctx.draw_elements(gl::TRIANGLES, &self.vertices, &self.indices);
    // pub fn draw_elements<V: DataSource, I: BufferIndex>(
    //     &mut self,
    //     primitive: PrimitiveType,
    //     program: &Program<V>,
    //     vertices: &Buffer<V>,
    //     indices: &Buffer<I>,
    // ) {
    //     if vertices.len() > 0 {
    //         program.bind(self);

    //         // Find a VAO that describes our vertex format, creating one if it
    // does not         // exist.
    //         let mut map = self.0.format_cache.borrow_mut();
    //         let vao = map.entry::<V>().or_insert_with(|| {
    //             VertexArray::<V>::for_vertex_type(self).with_buffer(self,
    // vertices)         });

    //         // set the buffer binding the the buffer that was passed in
    //         vao.set_buffer(self, vertices);
    //         vao.bind(self);
    //         indices.bind(BufferTarget::Element);

    //         gl_call!(assert DrawElements(
    //             primitive as u32,
    //             indices.len() as i32,
    //             I::INDEX_TYPE,
    //             0 as *const _
    //         ));
    //     }
    // }

    pub fn clear_color(&self, r: f32, g: f32, b: f32, a: f32) {
        gl_call!(assert ClearColor(r, g, b, a));
        gl_call!(assert Clear(gl::COLOR_BUFFER_BIT));
    }

    // self.ctx.draw_elements(gl::TRIANGLES, &shader, &self.vertices);
    pub fn draw_arrays<'v, V>(
        &mut self,
        primitive: PrimitiveType,
        program: &Program<V>,
        data: V::Buffers,
    ) where
        V: DataSource<'v> + 'static,
    {
        program.bind();

        let mut map = self.0.format_cache.borrow_mut();
        let vao = map
            .entry(TypeId::of::<V>())
            .or_insert_with(|| VertexArray::for_data_source::<V>(self));

        vao.bind();
        let len = V::apply_sources(data, &mut vao.binder());

        if let Some(len) = len {
            gl_call!(assert DrawArrays(primitive as u32, 0, (primitive.vertices_per_primitive() * len) as i32));
        }
    }

    pub fn limits(&self) -> &Limits {
        &self.0.limits
    }

    pub fn set_viewport<R>(&self, rect: R)
    where
        R: Into<ViewportRect>,
    {
        let rect = rect.into();
        self.0.viewport.set(rect);
        gl_call!(assert Viewport(rect.x, rect.y, rect.width as i32, rect.height as i32))
    }

    pub fn viewport(&self) -> ViewportRect {
        self.0.viewport.get()
    }
}
