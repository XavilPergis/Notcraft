use super::{super::error::GlError, shader::CompiledShader};
use gl::{self, types::*};
use gl_api::{context::Context, uniform::*};
use std::collections::HashMap;

pub struct Program {
    id: GLuint,
}

impl !Send for Program {}
impl !Sync for Program {}

impl Program {
    pub fn new() -> Option<Self> {
        // UNWRAP: this function never sets an error state
        let id = gl_call!(assert CreateProgram());
        match id {
            0 => None,
            id => Some(Program { id }),
        }
    }

    pub fn bind(&self) {
        gl_call!(assert UseProgram(self.id));
    }

    pub fn attach_shader(&self, shader: CompiledShader) {
        gl_call!(assert AttachShader(self.id, shader.shader.id));
    }

    pub fn link(self) -> Result<LinkedProgram, LinkError> {
        gl_call!(LinkProgram(self.id))?;
        check_program_status(self.id, gl::LINK_STATUS)?;
        gl_call!(ValidateProgram(self.id))?;
        check_program_status(self.id, gl::VALIDATE_STATUS)?;
        Ok(LinkedProgram {
            program: self,
            uniform_cache: HashMap::new(),
        })
    }
}

pub struct LinkedProgram {
    program: Program,
    uniform_cache: HashMap<String, UniformLocation>,
}

impl LinkedProgram {
    pub fn set_uniform<U: Uniform>(&mut self, ctx: &mut Context, name: &str, uniform: &U) {
        self.program.bind();
        uniform.set_uniform(
            ctx,
            if let Some(location) = self.uniform_cache.get(name) {
                *location
            } else {
                let location = self.get_uniform_location(name);
                self.uniform_cache.insert(name.into(), location);
                location
            },
        );
    }

    pub fn bind(&self) {
        self.program.bind();
    }

    fn get_uniform_location(&self, name: &str) -> UniformLocation {
        self.program.bind();
        use std::ffi::CString;
        let c_string = CString::new(name).unwrap();
        // UNWRAP: program ID is valid, and the program has been successfully linked
        gl_call!(assert GetUniformLocation(self.program.id, c_string.as_ptr()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum LinkError {
    Other(String),
    Gl(GlError),
}

impl From<::gl_api::error::GlError> for LinkError {
    fn from(err: ::gl_api::error::GlError) -> Self {
        LinkError::Gl(err)
    }
}

fn check_program_status(id: GLuint, ty: GLenum) -> Result<(), LinkError> {
    let mut status = 1;
    gl_call!(assert GetProgramiv(id, ty, &mut status));

    if status == 0 {
        Err(LinkError::Other(
            program_info_log(id).unwrap_or(String::new()),
        ))
    } else {
        Ok(())
    }
}

fn program_info_log(id: GLuint) -> Option<String> {
    let mut length = 0;
    gl_call!(assert GetProgramiv(id, gl::INFO_LOG_LENGTH, &mut length));
    if length == 0 {
        None
    } else {
        let mut buffer = Vec::<u8>::with_capacity(length as usize);
        gl_call!(assert GetProgramInfoLog(
            id,
            length,
            ::std::ptr::null_mut(),
            buffer.as_mut_ptr() as *mut i8
        ));

        unsafe {
            buffer.set_len((length - 1) as usize);
        }

        Some(String::from_utf8(buffer).expect("Program info log was not UTF-8"))
    }
}
