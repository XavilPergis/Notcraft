use crate::engine::prelude::*;
use anyhow::Context;
use glium::{
    backend::Facade, program::SourceCode, texture::TextureCreationError, Display, Program,
    ProgramCreationError,
};
use image::{GenericImageView, ImageError, RgbaImage};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs::File,
    io::ErrorKind,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

macro_rules! err_from {
    ($sup:ident => $sub:path = $variant:ident) => {
        impl From<$sub> for $sup {
            fn from(sub: $sub) -> Self {
                $sup::$variant(sub)
            }
        }
    };
}

#[derive(Debug)]
pub enum TextureLoadError {
    Io(std::io::Error),
    Image(ImageError),
    Texture(TextureCreationError),
    MismatchedDimensions(HashSet<(u32, u32)>),
}

impl std::error::Error for TextureLoadError {}
impl std::fmt::Display for TextureLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "texture load error: ")?;
        match self {
            TextureLoadError::Io(err) => write!(f, "io: {}", err)?,
            TextureLoadError::Image(err) => write!(f, "image: {}", err)?,
            TextureLoadError::Texture(err) => write!(f, "texture: {}", err)?,
            TextureLoadError::MismatchedDimensions(dims) => {
                write!(f, "mismatched dimensions: ")?;
                for (x, y) in dims {
                    write!(f, "({}, {}), ", x, y)?;
                }
            }
        }
        Ok(())
    }
}

err_from! { TextureLoadError => ImageError = Image }
err_from! { TextureLoadError => std::io::Error = Io }
err_from! { TextureLoadError => TextureCreationError = Texture }

#[derive(Clone, Debug)]
pub struct BlockTextures {
    pub width: u32,
    pub height: u32,
    pub unknown_texture: Arc<RgbaImage>,
    pub block_textures: HashMap<String, Arc<RgbaImage>>,
}

struct BlockTextureLoadContext<'env> {
    base_path: &'env Path,
    found_dimensions: HashSet<(u32, u32)>,
}

impl<'env> BlockTextureLoadContext<'env> {
    fn new(base_path: &'env Path) -> Self {
        Self {
            base_path,
            found_dimensions: Default::default(),
        }
    }

    fn load(&mut self, path: &str) -> Result<Option<RgbaImage>, TextureLoadError> {
        let texture_path = self.base_path.join(format!("{}.png", path));
        log::trace!("loading block texture from {}", texture_path.display());
        let image = match image::open(&texture_path) {
            Ok(image) => image,
            Err(ImageError::IoError(err)) if err.kind() == ErrorKind::NotFound => {
                log::warn!(
                    "block texture '{}' was not found in {}!",
                    path,
                    texture_path.display()
                );
                return Ok(None);
            }
            Err(other) => return Err(other.into()),
        };
        self.found_dimensions.insert(image.dimensions());
        Ok(Some(image.to_rgba()))
    }

    fn dimensions(&self) -> Option<(u32, u32)> {
        if self.found_dimensions.len() == 1 {
            self.found_dimensions.iter().copied().next()
        } else {
            None
        }
    }
}

pub fn load_block_textures<'a, P, I>(
    base_path: P,
    names: I,
) -> Result<BlockTextures, TextureLoadError>
where
    P: AsRef<Path>,
    I: IntoIterator<Item = &'a str>,
{
    let mut ctx = BlockTextureLoadContext::new(base_path.as_ref());

    let names = names.into_iter();

    let unknown_texture = Arc::new(ctx.load("unknown")?.unwrap());

    let mut block_textures = HashMap::new();
    for entry in names {
        let texture = ctx
            .load(entry)?
            .map(Arc::new)
            .unwrap_or_else(|| Arc::clone(&unknown_texture));
        block_textures.insert(entry.to_owned(), texture);
    }

    match ctx.dimensions() {
        Some((width, height)) => Ok(BlockTextures {
            width,
            height,
            unknown_texture,
            block_textures,
        }),
        None => Err(TextureLoadError::MismatchedDimensions(ctx.found_dimensions)),
    }
}

#[derive(Debug)]
pub enum ShaderLoadError {
    Io(std::io::Error),
    Program(ProgramCreationError),
    MissingFragment,
    MissingVertex,
}

impl std::error::Error for ShaderLoadError {}

impl std::fmt::Display for ShaderLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shader load error: ")?;
        match self {
            ShaderLoadError::Io(err) => write!(f, "{}", err),
            ShaderLoadError::Program(err) => write!(f, "{}", err),
            ShaderLoadError::MissingFragment => write!(f, "missing fragment stage"),
            ShaderLoadError::MissingVertex => write!(f, "missing vertex stage"),
        }
    }
}

err_from! { ShaderLoadError => std::io::Error = Io }
err_from! { ShaderLoadError => ProgramCreationError = Program }

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct ShaderId(usize);

pub struct ShaderLoaderState {
    display: Rc<Display>,

    paths: HashMap<String, Rc<PathBuf>>,
    infos: HashMap<PathBuf, PathInfo>,
    next_id: ShaderId,
    base_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ShaderManifest {
    paths: HashMap<String, PathBuf>,
}

impl ShaderLoaderState {
    pub fn new(display: &Rc<Display>, base_path: PathBuf) -> Result<Self> {
        let manifest_file = File::open(base_path.join("manifest.json"))?;
        let manifest: ShaderManifest = serde_json::from_reader(manifest_file)?;

        let paths = manifest
            .paths
            .into_iter()
            .map(|(name, path)| (name, Rc::new(base_path.join(path))))
            .collect();

        Ok(Self {
            display: Rc::clone(display),
            paths,
            infos: Default::default(),
            next_id: ShaderId(0),
            base_path,
        })
    }

    pub fn get(&mut self, name: &str) -> Result<Rc<Program>> {
        let path = match self.paths.get(name).map(Rc::clone) {
            Some(path) => path,
            None => bail!("unknown shader '{}'", name),
        };
        match self.info_mut(&path)?.program.as_ref() {
            Some(program) => Ok(Rc::clone(program)),
            None => load_shader_internal(self, path.as_ref())
                .with_context(|| anyhow!("error loading shader '{}'", path.display())),
        }
    }

    fn source(&mut self, path: &Path) -> Result<Arc<String>> {
        Ok(self.info_mut(path)?.raw_source.clone())
    }

    fn info_mut(&mut self, path: &Path) -> Result<&mut PathInfo> {
        if !self.infos.contains_key(path) {
            log::debug!("shader source for path '{}' was not cached", path.display());
            let source = std::fs::read_to_string(path)?;
            self.infos.insert(path.into(), PathInfo::new(source));
        }
        Ok(self.infos.get_mut(path).unwrap())
    }
}

#[derive(Debug)]
struct PathInfo {
    program: Option<Rc<Program>>,
    shader_id: Option<ShaderId>,
    raw_source: Arc<String>,

    // IDs of shaders that we included
    includes: HashSet<ShaderId>,
    // IDs of shaders that included us
    included_by: HashSet<ShaderId>,
}

impl PathInfo {
    pub fn new(source: String) -> Self {
        Self {
            raw_source: Arc::new(source),
            program: Default::default(),
            shader_id: Default::default(),
            includes: Default::default(),
            included_by: Default::default(),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    TesselationControl,
    TesselationEvaluation,
    Geometry,
    Compute,
}

impl ShaderStage {
    pub fn enumerate(mut func: impl FnMut(Self)) {
        func(Self::Vertex);
        func(Self::Fragment);
        func(Self::TesselationControl);
        func(Self::TesselationEvaluation);
        func(Self::Geometry);
        func(Self::Compute);
    }
}

#[derive(Clone, Debug)]
pub enum ShaderParseEvent<'src> {
    /// the beginning of the shader located at the specified path. this is
    /// emitted before anything else. likewise, [`End`] is emitted after
    /// everything else in the file.
    Start(&'src Path),
    End(&'src Path),

    Fragment(&'src str),

    ShaderStage(ShaderStage),
    Include(&'src Path),
}

#[derive(Debug)]
struct ShaderParser<'src> {
    path: &'src Path,
    source: &'src str,
    current: usize,

    fragments: Vec<ShaderParseEvent<'src>>,
    last_literal_start: usize,
    last_literal_end: usize,
    // if we're at the start of a line, or only have whitespace so far this line
    at_line_start: bool,
    errors: Vec<anyhow::Error>,
}

impl<'src> ShaderParser<'src> {
    pub fn new(path: &'src Path, source: &'src str) -> Self {
        Self {
            path,
            source,
            current: 0,
            fragments: Default::default(),
            last_literal_start: 0,
            last_literal_end: 0,
            at_line_start: true,
            errors: Default::default(),
        }
    }
}

impl<'src> ShaderParser<'src> {
    fn peek(&self) -> Option<char> {
        self.source[self.current..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.source[self.current..].chars().next()?;
        match ch {
            '\n' => self.at_line_start = true,
            ch if !ch.is_ascii_whitespace() => self.at_line_start = false,
            _ => {}
        }
        self.current += ch.len_utf8();
        Some(ch)
    }

    fn advance_until<F>(&mut self, mut func: F) -> &'src str
    where
        F: FnMut(char) -> bool,
    {
        self.advance_while(|ch| !func(ch))
    }

    fn advance_while<F>(&mut self, mut func: F) -> &'src str
    where
        F: FnMut(char) -> bool,
    {
        let start = self.current;
        while let Some(ch) = self.peek() {
            if !func(ch) {
                break;
            }
            self.advance();
        }
        &self.source[start..self.current]
    }

    fn advance_if<F>(&mut self, mut func: F) -> bool
    where
        F: FnMut(char) -> bool,
    {
        let start = self.current;
        if let Some(ch) = self.peek() {
            if func(ch) {
                self.advance();
            }
        }
        start != self.current
    }
}

fn parse_maybe_comment<'src>(parser: &mut ShaderParser<'src>) {
    match parser.peek() {
        // line comment
        Some('/') => {
            parser.advance_until(|ch| ch == '\r' || ch == '\n');
            parser.advance_if(|ch| ch == '\r');
            parser.advance_if(|ch| ch == '\n');
        }
        // block comment. note that you can't have nested block comments in GLSL.
        Some('*') => loop {
            parser.advance_until(|ch| ch == '*');
            parser.advance();
            match parser.peek() {
                None | Some('/') => break,
                _ => {}
            }
        },
        _ => {}
    }
}

fn submit_literal_parse_event<'src>(parser: &mut ShaderParser<'src>) {
    if parser.last_literal_start != parser.current {
        parser.fragments.push(ShaderParseEvent::Fragment(
            &parser.source[parser.last_literal_start..parser.last_literal_end],
        ));
        parser.last_literal_start = parser.current;
        parser.last_literal_end = parser.current;
    }
}

fn add_shader_parse_event<'src>(parser: &mut ShaderParser<'src>, event: ShaderParseEvent<'src>) {
    submit_literal_parse_event(parser);
    parser.fragments.push(event);
}

fn parse_include_directive<'src>(parser: &mut ShaderParser<'src>) {
    parser.advance_while(|ch| ch.is_ascii_whitespace());
    if parser.advance_if(|ch| ch == '"') {
        let path = parser.advance_until(|ch| ch == '"');
        parser.advance();

        add_shader_parse_event(parser, ShaderParseEvent::Include(path.as_ref()));
    }
}

fn parse_shaderstage_directive<'src>(parser: &mut ShaderParser<'src>) -> Result<()> {
    parser.advance_while(|ch| ch.is_ascii_whitespace());
    let stage = match parser.advance_while(|ch| ch == '_' || ch.is_ascii_alphabetic()) {
        "vertex" => ShaderStage::Vertex,
        "fragment" => ShaderStage::Fragment,
        "tesselation_control" => ShaderStage::TesselationControl,
        "tesselation_evaluation" => ShaderStage::TesselationEvaluation,
        "geometry" => ShaderStage::Geometry,
        "compute" => ShaderStage::Compute,
        other => bail!("unknown shader stage '{}'", other),
    };

    add_shader_parse_event(parser, ShaderParseEvent::ShaderStage(stage));
    Ok(())
}

fn parse_directive<'src>(parser: &mut ShaderParser<'src>) -> Result<()> {
    parser.advance_while(|ch| ch.is_ascii_whitespace());

    let directive = parser.advance_while(|ch| ch == '_' || ch.is_ascii_alphabetic());
    if directive != "pragma" {
        return Ok(());
    }

    parser.advance_while(|ch| ch.is_ascii_whitespace());
    match parser.advance_while(|ch| ch == '_' || ch.is_ascii_alphabetic()) {
        "include" => parse_include_directive(parser),
        "shaderstage" => parse_shaderstage_directive(parser)?,
        other => bail!("unknown pragma directive '{}'", other),
    }

    Ok(())
}

fn visit_fragments_at_path<'src, F>(
    state: &mut ShaderLoaderState,
    path: &Path,
    visitor: &mut F,
) -> Result<()>
where
    F: FnMut(&ShaderParseEvent<'_>),
{
    let source = state.source(path)?;
    let mut parser = ShaderParser::new(path.as_ref(), &source);
    parse_shader(&mut parser)?;

    log::debug!("parsed shader '{}'", path.display());

    for fragment in parser.fragments.iter() {
        log::debug!("fragment {:?}", fragment);
        match fragment {
            &ShaderParseEvent::Include(include_path) => {
                // the include path is relative to the file that the include occurred in, which
                // is what the `path` parameter is.

                // allowing arbitrary filesystem acces here would probably be a bad idea lol. we
                // reject a path if it's in a directory higher up than `state`'s `base_path`
                let canonical = state.base_path.join(include_path).canonicalize()?;
                if state.base_path.ancestors().any(|path| path == canonical) {
                    bail!(
                        "tried to include '{}', which is outside of the base shader directory of '{}'",
                        canonical.display(),
                        state.base_path.display()
                    );
                }

                visit_fragments_at_path(state, &canonical, visitor)?;
            }
            event => visitor(event),
        }
    }
    Ok(())
}

fn emit_shader_code(
    state: &mut ShaderLoaderState,
    path: &Path,
) -> Result<HashMap<ShaderStage, String>> {
    let mut res: HashMap<ShaderStage, String> = HashMap::new();
    let mut stage_stack = Vec::new();
    let mut current_stage = None;

    visit_fragments_at_path(state, path, &mut |event| match event {
        &ShaderParseEvent::Include(_) => unreachable!(),
        &ShaderParseEvent::Fragment(src) => match current_stage {
            Some(stage) => res.entry(stage).or_default().push_str(src),
            None => ShaderStage::enumerate(|stage| {
                if let Some(res) = res.get_mut(&stage) {
                    res.push_str(src);
                }
            }),
        },
        &ShaderParseEvent::Start(path) => {
            stage_stack.push(current_stage);
            current_stage = None;
        }
        &ShaderParseEvent::End(_) => current_stage = stage_stack.pop().unwrap(),
        &ShaderParseEvent::ShaderStage(stage) => current_stage = Some(stage),
    })?;

    Ok(res)
}

fn parse_shader<'src>(parser: &mut ShaderParser<'src>) -> Result<()> {
    add_shader_parse_event(parser, ShaderParseEvent::Start(parser.path));

    while let Some(ch) = parser.peek() {
        let at_line_start = parser.at_line_start;
        parser.last_literal_end = parser.current;
        parser.advance();
        match ch {
            '/' => parse_maybe_comment(parser),
            '#' if at_line_start => {
                if let Err(err) = parse_directive(parser) {
                    parser.errors.push(err);
                }
            }
            _ => {}
        }
    }

    add_shader_parse_event(parser, ShaderParseEvent::End(parser.path));

    if !parser.errors.is_empty() {
        for error in parser.errors.iter() {
            log::error!("error in shader '{}': {}", parser.path.display(), error);
        }
        bail!(
            "{} errors encountered in shader '{}'",
            parser.errors.len(),
            parser.path.display()
        );
    }

    Ok(())
}

fn load_shader_internal(state: &mut ShaderLoaderState, path: &Path) -> Result<Rc<Program>> {
    let source = emit_shader_code(state, path)?;
    for (stage, src) in source.iter() {
        log::debug!("emitted for stage {:?}: \n\n{}\n\n", stage, src);
    }
    let program = Rc::new(Program::new(&*state.display, SourceCode {
        vertex_shader: source
            .get(&ShaderStage::Vertex)
            .ok_or_else(|| anyhow!("shader '{}' is missing a vertex stage", path.display()))?,
        fragment_shader: source
            .get(&ShaderStage::Fragment)
            .ok_or_else(|| anyhow!("shader '{}' is missing a fragment stage", path.display()))?,
        tessellation_control_shader: source.get(&ShaderStage::TesselationControl).map(|s| &**s),
        tessellation_evaluation_shader: source
            .get(&ShaderStage::TesselationEvaluation)
            .map(|s| &**s),
        geometry_shader: source.get(&ShaderStage::Geometry).map(|s| &**s),
    })?);

    state.info_mut(path)?.program = Some(Rc::clone(&program));
    Ok(program)
}

pub fn load_shader<P: AsRef<Path>, F: Facade>(
    ctx: &F,
    path: P,
) -> Result<Program, ShaderLoadError> {
    let dir = path.as_ref().read_dir()?;

    let mut vertex = None;
    let mut fragment = None;
    let mut geometry = None;
    let mut tess_eval = None;
    let mut tess_control = None;

    for file in dir {
        let file = file?;
        if file.file_type()?.is_file() {
            let path = file.path();

            match path.extension().and_then(OsStr::to_str) {
                Some("vert") => vertex = Some(std::fs::read_to_string(&path)?),
                Some("tesc") => tess_control = Some(std::fs::read_to_string(&path)?),
                Some("tese") => tess_eval = Some(std::fs::read_to_string(&path)?),
                Some("geom") => geometry = Some(std::fs::read_to_string(&path)?),
                Some("frag") => fragment = Some(std::fs::read_to_string(&path)?),

                _ => (),
            }
        }
    }

    match (vertex, fragment) {
        (None, _) => Err(ShaderLoadError::MissingVertex),
        (_, None) => Err(ShaderLoadError::MissingFragment),
        (Some(vert), Some(frag)) => Ok(Program::new(ctx, SourceCode {
            vertex_shader: &vert,
            tessellation_control_shader: tess_control.as_ref().map(|src| &**src),
            tessellation_evaluation_shader: tess_eval.as_ref().map(|src| &**src),
            geometry_shader: geometry.as_ref().map(|src| &**src),
            fragment_shader: &frag,
        })?),
    }
}
