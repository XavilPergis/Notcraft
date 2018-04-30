use std::path::Path;
use std::fs::File;
use std::io::{self, Read};

pub mod program;
pub mod shader;

use shader::*;
use program::*;

#[derive(Debug)]
pub enum PipelineError {
    Shader(ShaderError),
    Io(io::Error),
    ProgramCreation,
}

impl From<ShaderError> for PipelineError {
    fn from(err: ShaderError) -> Self { PipelineError::Shader(err) }
}

impl From<io::Error> for PipelineError {
    fn from(err: io::Error) -> Self { PipelineError::Io(err) }
}

pub fn simple_pipeline<P1: AsRef<Path>, P2: AsRef<Path>>(vert: P1, frag: P2) -> Result<LinkedProgram, PipelineError> {
    let program = Program::new().ok_or(PipelineError::ProgramCreation)?;
    let vert_shader = Shader::new(ShaderType::Vertex)?;
    let frag_shader = Shader::new(ShaderType::Fragment)?;

    vert_shader.source_from_file(vert)?;
    frag_shader.source_from_file(frag)?;

    program.attach_shader(vert_shader.compile()?);
    program.attach_shader(frag_shader.compile()?);

    Ok(program.link().unwrap())
}
