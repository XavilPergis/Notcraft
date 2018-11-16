use gl;
use gl::types::*;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::ptr;

fn shader_info_log(shader: &Shader) -> Option<String> {
    let id = shader.id;
    let mut length = 0;
    gl_call!(assert GetShaderiv(id, gl::INFO_LOG_LENGTH, &mut length));

    if length == 0 {
        None
    } else {
        let mut buffer = Vec::<u8>::with_capacity(length as usize);
        // TODO: unwrap
        gl_call!(assert GetShaderInfoLog(
            id,
            length,
            ptr::null_mut(),
            buffer.as_mut_ptr() as *mut i8
        ));

        unsafe {
            buffer.set_len((length - 1) as usize);
        }

        Some(String::from_utf8(buffer).expect("Shader info log was not UTF-8"))
    }
}

#[allow(dead_code)]
#[repr(u32)]
pub enum ShaderType {
    Vertex = gl::VERTEX_SHADER,
    Fragment = gl::FRAGMENT_SHADER,
    Geometry = gl::GEOMETRY_SHADER,
}

#[derive(Debug)]
pub enum ShaderError {
    Creation,
    Shader(String),
    Io(io::Error),
}

pub type ShaderResult<T> = Result<T, ShaderError>;

impl From<io::Error> for ShaderError {
    fn from(err: io::Error) -> Self {
        ShaderError::Io(err)
    }
}

pub struct Shader {
    pub(super) id: GLuint,
}

impl Shader {
    pub fn new(shader_type: ShaderType) -> ShaderResult<Self> {
        let id = gl_call!(assert CreateShader(shader_type as u32));
        if id == 0 {
            return Err(ShaderError::Creation);
        }
        Ok(Shader { id })
    }

    pub fn source_from_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut buf = String::new();
        let mut file = File::open(path)?;
        file.read_to_string(&mut buf)?;
        self.shader_source(buf);
        Ok(())
    }

    pub fn shader_source<S: AsRef<[u8]>>(&self, source: S) {
        let source = source.as_ref();
        gl_call!(assert ShaderSource(
            self.id,
            1,
            &(source.as_ptr() as *const GLchar),
            &(source.len() as i32)
        ));
    }

    pub fn compile(self) -> ShaderResult<CompiledShader> {
        // UNWRAP: `self.id` is a valid shader handle, and no other GL errors are emitted here;
        // the compile status is part of the Shader object's state
        let mut status = 1;
        gl_call!(assert CompileShader(self.id));
        gl_call!(assert GetShaderiv(self.id, gl::COMPILE_STATUS, &mut status));
        if status == 0 {
            let log = shader_info_log(&self).unwrap();
            Err(ShaderError::Shader(log))
        } else {
            Ok(CompiledShader { shader: self })
        }
    }
    // TODO: pub fn shader_source_many(&self, )
}

impl Drop for Shader {
    fn drop(&mut self) {
        gl_call!(debug DeleteShader(self.id));
    }
}

pub struct CompiledShader {
    pub(super) shader: Shader,
}
