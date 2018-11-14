use gl;

pub type GlResult<T> = Result<T, GlError>;

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct GlError {
    code: gl::types::GLenum,
}

impl GlError {
    pub fn get() -> Self {
        GlError {
            code: unsafe { gl::GetError() },
        }
    }

    pub fn result<T>(self, res: T) -> GlResult<T> {
        match self.code {
            0 => Ok(res),
            _ => Err(self),
        }
    }
}

impl std::fmt::Debug for GlError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(
            f,
            "0x{:X} {}",
            self.code,
            match self.code {
                gl::INVALID_ENUM => "GL_INVALID_ENUM",
                gl::INVALID_VALUE => "GL_INVALID_VALUE",
                gl::INVALID_OPERATION => "GL_INVALID_OPERATION",
                gl::STACK_OVERFLOW => "GL_STACK_OVERFLOW",
                gl::STACK_UNDERFLOW => "GL_STACK_UNDERFLOW",
                gl::OUT_OF_MEMORY => "GL_OUT_OF_MEMORY",
                gl::INVALID_FRAMEBUFFER_OPERATION => "GL_INVALID_FRAMEBUFFER_OPERATION",
                _ => "unknown",
            }
        )
    }
}

macro_rules! gl_call {
    ($name:ident($($args:expr),*)) => {{
        use ::gl;
        let ret = unsafe { gl::$name($($args),*) };
        $crate::gl_api::error::GlError::get().result(ret)
    }};

    (assert $name:ident($($args:expr),*)) => {{
        use ::gl;
        let ret = unsafe { gl::$name($($args),*) };
        match $crate::gl_api::error::GlError::get().result(ret) {
            Err(err) => panic!("gl{} failed with error code {:?}", stringify!($name), err),
            Ok(res) => res
        }
    }};

    (debug $name:ident($($args:expr),*)) => {{
        use ::gl;
        let ret = unsafe { gl::$name($($args),*) };
        if cfg!(debug_assertions) {
            if $crate::gl_api::error::GlError::get().result(ret).is_err() {
                panic!("gl{} failed with error code {:?}");
            }
        }
        ret
    }}
}
