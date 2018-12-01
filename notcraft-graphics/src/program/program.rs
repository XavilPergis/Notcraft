use crate::{
    context::Context,
    program::{
        shader::{Stage, StageType},
        uniform::*,
        Compiled,
    },
};
use gl::{self, types::*};
use std::{collections::HashMap, marker::PhantomData};

pub struct RawProgram {
    id: u32,
}

impl RawProgram {
    pub fn new(_ctx: &Context) -> Self {
        let id = gl_call!(assert CreateProgram());
        assert!(id > 0);
        RawProgram { id }
    }
}

impl Drop for RawProgram {
    fn drop(&mut self) {
        gl_call!(debug DeleteProgram(self.id));
    }
}

pub struct ProgramStages {
    ctx: Context,
    vertex: Stage,
    fragment: Stage,
    geometry: Option<Stage>,
    tesselation: Option<(Stage, Stage)>,
}

impl ProgramStages {
    pub fn new<Vert, Frag>(ctx: &Context, vertex: Vert, fragment: Frag) -> Self
    where
        Vert: AsRef<str>,
        Frag: AsRef<str>,
    {
        ProgramStages {
            ctx: ctx.clone(),
            vertex: Stage::new(ctx, StageType::Vertex, vertex.as_ref()),
            fragment: Stage::new(ctx, StageType::Fragment, fragment.as_ref()),
            geometry: None,
            tesselation: None,
        }
    }

    pub fn build(self) -> Result<Compiled<ProgramStages>, (StageType, String)> {
        self.vertex.compile()?;
        self.fragment.compile()?;

        if let Some(geometry) = self.geometry.as_ref() {
            geometry.compile()?;
        }

        if let Some(&(ref eval, ref control)) = self.tesselation.as_ref() {
            eval.compile()?;
            control.compile()?;
        }

        Ok(Compiled(self))
    }
}

pub fn create_program<I>(
    ctx: &Context,
    stages: Compiled<ProgramStages>,
) -> Result<Program<I>, String> {
    let raw = RawProgram::new(ctx);

    gl_call!(assert AttachShader(raw.id, stages.0.vertex.id));
    gl_call!(assert AttachShader(raw.id, stages.0.fragment.id));

    if let Some(stage) = stages.0.geometry {
        gl_call!(assert AttachShader(raw.id, stage.id));
    }

    if let Some((eval, control)) = stages.0.tesselation {
        gl_call!(assert AttachShader(raw.id, eval.id));
        gl_call!(assert AttachShader(raw.id, control.id));
    }

    gl_call!(assert LinkProgram(raw.id));
    check_program_status(raw.id, gl::LINK_STATUS)?;
    gl_call!(assert ValidateProgram(raw.id));
    check_program_status(raw.id, gl::VALIDATE_STATUS)?;

    Ok(Program {
        raw,
        uniform_cache: HashMap::new(),
        _marker: PhantomData,
    })
}

pub struct Program<I> {
    raw: RawProgram,
    uniform_cache: HashMap<String, UniformLocation>,
    _marker: PhantomData<*const I>,
}

impl<I> Program<I> {
    pub fn set_uniform<U: Uniform>(&mut self, ctx: &Context, name: &str, uniform: &U) {
        self.bind();
        if let Some(&location) = self.uniform_cache.get(name) {
            uniform.set_uniform(ctx, location);
        } else {
            let location = self.get_uniform_location(name);
            self.uniform_cache.insert(name.into(), location);
            uniform.set_uniform(ctx, location);
        }
    }

    pub fn bind(&self) {
        gl_call!(assert UseProgram(self.raw.id));
    }

    fn get_uniform_location(&self, name: &str) -> UniformLocation {
        use std::ffi::CString;
        let c_string = CString::new(name).unwrap();
        // UNWRAP: program ID is valid, and the program has been successfully linked
        gl_call!(assert GetUniformLocation(self.raw.id, c_string.as_ptr()))
    }
}

fn check_program_status(id: GLuint, ty: GLenum) -> Result<(), String> {
    let mut status = 1;
    gl_call!(assert GetProgramiv(id, ty, &mut status));

    if status == 0 {
        Err(program_info_log(id).unwrap_or_default())
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
            ::std::ptr::null_mut::<i32>(),
            buffer.as_mut_ptr() as *mut i8
        ));

        unsafe {
            buffer.set_len((length - 1) as usize);
        }

        Some(String::from_utf8(buffer).expect("Program info log was not UTF-8"))
    }
}
