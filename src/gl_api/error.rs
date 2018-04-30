use gl;

pub type GlResult<T> = Result<T, GlError>;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct GlError {
    code: gl::types::GLenum,
}

impl GlError {
    pub fn get() -> Option<Self> {
        unsafe {
            match gl::GetError() {
                0 => None,
                code => Some(GlError { code })
            }
        }
    }
}

macro_rules! gl_call {
    ($name:ident($($args:expr),*)) => {{
        use ::gl;
        let ret = gl::$name($($args),*);
        match $crate::gl_api::error::GlError::get() {
            Some(err) => Err(err),
            None => Ok(ret),
        }
    }}
}
