macro_rules! define_raw_object {
    ($name:ident, $genfn:ident, $delfn:ident) => {
        #[derive(Debug)]
        pub struct $name {
            crate ctx: ::gl_api::Context,
            crate id: u32,
        }

        impl $name {
            pub fn new(ctx: &::gl_api::Context) -> Self {
                let mut id = 0;
                gl_call!(assert $genfn(1, &mut id));

                $name {
                    ctx: ctx.clone(),
                    id,
                }
            }
        }

        impl Drop for $name {
            fn drop(&mut self) {
                gl_call!(debug $delfn(1, &mut self.id));
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &$name) -> bool {
                self.id == other.id
            }
        }

        impl Eq for $name {}
    };
}

define_raw_object!(RawVertexArray, CreateVertexArrays, DeleteVertexArrays);
define_raw_object!(RawBuffer, CreateBuffers, DeleteBuffers);
define_raw_object!(RawFramebuffer, CreateFramebuffers, DeleteFramebuffers);
