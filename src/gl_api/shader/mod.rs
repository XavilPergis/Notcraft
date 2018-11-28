use gl_api::context::Context;
use std::{io, path::Path};

pub mod program;
pub mod shader;
pub mod uniform;

use program::*;
use shader::*;

#[derive(Debug)]
pub enum PipelineError {
    Shader(ShaderError),
    Io(io::Error),
    ProgramCreation,
}

impl From<ShaderError> for PipelineError {
    fn from(err: ShaderError) -> Self {
        PipelineError::Shader(err)
    }
}

impl From<io::Error> for PipelineError {
    fn from(err: io::Error) -> Self {
        PipelineError::Io(err)
    }
}

pub fn load_shader<P1: AsRef<Path>, P2: AsRef<Path>>(ctx: &Context, vert: P1, frag: P2) -> Program {
    match simple_pipeline(ctx, vert, frag) {
        Ok(program) => program,
        Err(PipelineError::Shader(ShaderError::Shader(msg))) => {
            println!("Shader compilation error: {}", msg);
            panic!();
        }
        Err(PipelineError::Io(err)) | Err(PipelineError::Shader(ShaderError::Io(err))) => {
            println!("Shader compilation error: I/O {:?}", err);
            panic!();
        }
        Err(other) => panic!(other),
    }
}

pub fn simple_pipeline<P1: AsRef<Path>, P2: AsRef<Path>>(
    ctx: &Context,
    vert: P1,
    frag: P2,
) -> Result<Program, PipelineError> {
    let program = ProgramBuilder::new(ctx);
    let vert_shader = Shader::new(ShaderType::Vertex)?;
    let frag_shader = Shader::new(ShaderType::Fragment)?;

    vert_shader.source_from_file(vert)?;
    frag_shader.source_from_file(frag)?;

    program.attach_shader(vert_shader.compile()?);
    program.attach_shader(frag_shader.compile()?);

    Ok(program.link().unwrap())
}
