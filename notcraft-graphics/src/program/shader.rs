use crate::context::Context;
use gl;
use std::ptr;

pub struct Stage {
    pub(crate) id: u32,
    pub(crate) ty: StageType,
}

impl Stage {
    pub fn new(_ctx: &Context, ty: StageType, src: &str) -> Self {
        let id = gl_call!(assert CreateShader(ty as u32));
        assert!(id > 0);
        gl_call!(assert ShaderSource(id, 1, &(src.as_ptr() as *const _), &(src.len() as i32)));
        Stage { id, ty }
    }

    pub fn compile(&self) -> Result<(), (StageType, String)> {
        let mut status = 0;

        gl_call!(assert CompileShader(self.id));
        gl_call!(assert GetShaderiv(self.id, gl::COMPILE_STATUS, &mut status));

        if status == 0 {
            return Err((self.ty, shader_info_log(&self).unwrap_or_default()));
        }

        Ok(())
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        gl_call!(debug DeleteShader(self.id));
    }
}

fn shader_info_log(shader: &Stage) -> Option<String> {
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
            ptr::null_mut::<i32>(),
            buffer.as_mut_ptr() as *mut i8
        ));

        unsafe {
            buffer.set_len((length - 1) as usize);
        }

        Some(String::from_utf8(buffer).expect("Shader info log was not UTF-8"))
    }
}

#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum StageType {
    Vertex = gl::VERTEX_SHADER,
    Geometry = gl::GEOMETRY_SHADER,
    TessEval = gl::TESS_EVALUATION_SHADER,
    TessControl = gl::TESS_CONTROL_SHADER,
    Fragment = gl::FRAGMENT_SHADER,
}
