use crate::util::ChannelPair;
use glium::{program::SourceCode, texture::TextureCreationError, Display, Program};
use image::{GenericImageView, ImageError, RgbaImage};
use notcraft_common::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use super::render::renderer::RenderStage;

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
    pub block_textures: HashMap<PathBuf, Arc<RgbaImage>>,
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

    fn load(&mut self, path: &Path) -> Result<Option<RgbaImage>, TextureLoadError> {
        let texture_path = self.base_path.join(path);
        log::trace!("loading block texture from {}", texture_path.display());
        let image = match image::open(&texture_path) {
            Ok(image) => image,
            Err(ImageError::IoError(err)) if err.kind() == ErrorKind::NotFound => {
                log::warn!(
                    "block texture '{}' was not found in {}!",
                    path.display(),
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
    paths: I,
) -> Result<BlockTextures, TextureLoadError>
where
    P: AsRef<Path>,
    I: IntoIterator<Item = &'a Path>,
{
    let mut ctx = BlockTextureLoadContext::new(base_path.as_ref());

    let paths = paths.into_iter();

    let unknown_texture = Arc::new(ctx.load(Path::new("unknown.png"))?.unwrap());

    let mut block_textures = HashMap::new();
    for path in paths {
        let texture = ctx
            .load(path)?
            .map(Arc::new)
            .unwrap_or_else(|| Arc::clone(&unknown_texture));
        block_textures.insert(path.to_owned(), texture);
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

pub fn load_texture<P: AsRef<Path>>(path: P) -> Result<RgbaImage> {
    Ok(image::open(path)?.to_rgba())
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct ShaderId(usize);

pub struct ShaderLoaderState {
    display: Rc<Display>,

    norm_paths: HashMap<String, Rc<PathBuf>>,
    id_to_norm_path: HashMap<ShaderId, PathBuf>,
    norm_path_to_id: HashMap<PathBuf, ShaderId>,

    infos: HashMap<ShaderId, PathInfo>,
    next_id: ShaderId,
    base_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ShaderManifest {
    paths: HashMap<String, PathBuf>,
}

impl ShaderLoaderState {
    pub fn load(display: &Rc<Display>, base_path: PathBuf) -> Result<Self> {
        let base_path = base_path.canonicalize()?;
        let manifest_file = File::open(base_path.join("manifest.json"))?;
        let manifest: ShaderManifest = serde_json::from_reader(manifest_file)?;

        let mut state = Self {
            display: Rc::clone(display),
            norm_paths: Default::default(),
            id_to_norm_path: Default::default(),
            norm_path_to_id: Default::default(),
            infos: Default::default(),
            next_id: ShaderId(0),
            base_path,
        };

        for (name, rel_path) in manifest.paths.into_iter() {
            log::trace!("loading shader '{}' at path '{}'", name, rel_path.display());
            let norm_path = state.normalize_path(&rel_path)?;
            load_path(&mut state, norm_path.clone(), true)?;
            state.norm_paths.insert(name, Rc::new(norm_path));
        }

        Ok(state)
    }

    pub fn get(&mut self, name: &str) -> Result<Rc<Program>> {
        let norm_path = match self.norm_paths.get(name).map(Rc::clone) {
            Some(path) => path,
            None => bail!("unknown shader '{}'", name),
        };
        assert!(norm_path.is_relative());

        let id = self.id(&norm_path)?;
        match self.infos[&id].program.as_ref() {
            Some(program) => Ok(Rc::clone(program)),
            None => {
                load_shader_internal(self, id)?;
                Ok(Rc::clone(self.infos[&id].program.as_ref().unwrap()))
            }
        }
    }

    fn source(&mut self, id: ShaderId) -> Arc<String> {
        self.infos[&id].raw_source.clone()
    }

    fn info_mut(&mut self, id: ShaderId) -> &mut PathInfo {
        self.infos.get_mut(&id).unwrap()
    }

    fn id(&mut self, norm_path: &Path) -> Result<ShaderId> {
        match self.norm_path_to_id.get(norm_path) {
            Some(&id) => Ok(id),
            None => {
                log::trace!("path '{}' was not cached, adding.", norm_path.display());
                load_path(self, norm_path.into(), false)
            }
        }
    }

    fn normalize_path(&self, rel_path: &Path) -> Result<PathBuf> {
        let mut normalized = PathBuf::default();
        for component in rel_path.components() {
            match component {
                Component::Prefix(_) => unimplemented!(),
                Component::RootDir => {
                    normalized.clear();
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    normalized.pop();
                }
                Component::Normal(segment) => {
                    normalized.push(segment);
                }
            }
        }

        // don't allow unsanitized relative paths to escape the shader base directory
        // and attempt to load arbitrary files.
        if self.base_path.ancestors().any(|p| p == normalized) {
            bail!(
                "tried to load '{}', which is outside of the base shader directory of '{}'",
                normalized.display(),
                self.base_path.display()
            );
        }

        Ok(normalized)
    }

    fn relativize_path<'p>(&self, path: &'p Path) -> Result<&'p Path> {
        let res = path.strip_prefix(&self.base_path)?;
        Ok(res)
    }
}

fn load_path(state: &mut ShaderLoaderState, norm_path: PathBuf, compile: bool) -> Result<ShaderId> {
    assert!(!state.norm_path_to_id.contains_key(&norm_path));

    let id = state.next_id;
    state.next_id.0 += 1;

    let real_path = state.base_path.join(&norm_path);
    let file_data = match std::fs::read_to_string(&real_path) {
        Ok(data) => data,
        Err(err) => {
            log::error!(
                "failed to read path '{}'! (real path is '{}')",
                norm_path.display(),
                real_path.display()
            );
            return Err(err.into());
        }
    };

    let info = PathInfo::new(id, file_data, compile);
    state.infos.insert(id, info);
    state.id_to_norm_path.insert(id, norm_path.clone());
    state.norm_path_to_id.insert(norm_path, id);

    Ok(id)
}

#[derive(Debug)]
struct PathInfo {
    program: Option<Rc<Program>>,
    raw_source: Arc<String>,
    id: ShaderId,
    compile: bool,

    dependencies: HashSet<ShaderId>,
    dependants: HashSet<ShaderId>,
}

impl PathInfo {
    pub fn new(id: ShaderId, source: String, compile: bool) -> Self {
        Self {
            id,
            compile,
            raw_source: Arc::new(source),
            program: Default::default(),
            dependencies: Default::default(),
            dependants: Default::default(),
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
    /// the beginning of the shader located at the path specified in the parser
    /// these events came from. this is emitted before anything else.
    /// likewise, [`Self::End`] is emitted after everything else in the file.
    Start,
    End,

    Fragment(&'src str),

    ShaderStage(ShaderStage),
    Include(&'src Path),
}

#[derive(Debug)]
struct ShaderParser<'src, 'path> {
    norm_path: &'path Path,
    source: &'src str,
    current: usize,

    events: Vec<ShaderParseEvent<'src>>,
    last_literal_start: usize,
    last_literal_end: usize,
    // if we're at the start of a line, or only have whitespace so far this line
    at_line_start: bool,
    errors: Vec<anyhow::Error>,
}

impl<'src, 'path> ShaderParser<'src, 'path> {
    pub fn new(norm_path: &'path Path, source: &'src str) -> Self {
        Self {
            norm_path,
            source,
            current: 0,
            events: Default::default(),
            last_literal_start: 0,
            last_literal_end: 0,
            at_line_start: true,
            errors: Default::default(),
        }
    }
}

impl<'src, 'path> ShaderParser<'src, 'path> {
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

fn parse_maybe_comment<'src>(parser: &mut ShaderParser<'src, '_>) {
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

fn submit_literal_parse_event<'src>(parser: &mut ShaderParser<'src, '_>) {
    if parser.last_literal_start != parser.current {
        parser.events.push(ShaderParseEvent::Fragment(
            &parser.source[parser.last_literal_start..parser.last_literal_end],
        ));
        parser.last_literal_start = parser.current;
        parser.last_literal_end = parser.current;
    }
}

fn add_shader_parse_event<'src>(
    parser: &mut ShaderParser<'src, '_>,
    event: ShaderParseEvent<'src>,
) {
    submit_literal_parse_event(parser);
    parser.events.push(event);
}

fn parse_include_directive<'src>(parser: &mut ShaderParser<'src, '_>) {
    parser.advance_while(|ch| ch.is_ascii_whitespace());
    if parser.advance_if(|ch| ch == '"') {
        let rel_path = parser.advance_until(|ch| ch == '"');
        parser.advance();

        add_shader_parse_event(parser, ShaderParseEvent::Include(rel_path.as_ref()));
    }
}

fn parse_shaderstage_directive<'src>(parser: &mut ShaderParser<'src, '_>) -> Result<()> {
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

fn parse_directive<'src>(parser: &mut ShaderParser<'src, '_>) -> Result<()> {
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

fn declare_dependency(state: &mut ShaderLoaderState, id: ShaderId, dependency_id: ShaderId) {
    state.info_mut(id).dependencies.insert(dependency_id);
    state.info_mut(dependency_id).dependants.insert(id);
}

fn clear_dependencies(state: &mut ShaderLoaderState, id: ShaderId) {
    let mut dependencies = std::mem::take(&mut state.infos.get_mut(&id).unwrap().dependencies);
    for dependency in dependencies.iter() {
        state
            .infos
            .get_mut(&dependency)
            .unwrap()
            .dependants
            .remove(&id);
    }
    dependencies.clear();
    state.infos.get_mut(&id).unwrap().dependencies = dependencies;
}

fn visit_fragments_at_path<'src, F>(
    state: &mut ShaderLoaderState,
    id: ShaderId,
    visitor: &mut F,
) -> Result<()>
where
    F: FnMut(&ShaderParseEvent<'_>),
{
    let source = state.source(id);
    let events = {
        let norm_path = &state.id_to_norm_path[&id];
        let mut parser = ShaderParser::new(norm_path.as_ref(), &source);
        parse_shader(&mut parser)?
    };

    clear_dependencies(state, id);
    for fragment in events.iter() {
        match fragment {
            &ShaderParseEvent::Include(include_rel_path) => {
                // the include path is relative to the file that the include occurred in.
                let include_rel_path = state.id_to_norm_path[&id]
                    .parent()
                    .unwrap()
                    .join(include_rel_path);
                let include_norm_path = state.normalize_path(&include_rel_path)?;
                let included_id = state.id(&include_norm_path)?;
                declare_dependency(state, id, included_id);
                visit_fragments_at_path(state, included_id, visitor)?;
            }
            event => visitor(event),
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Default)]
struct ProcessedShaderCode {
    // code that wasn't written inside a stage declaration, and should be prepended to all defined
    // stages. useful for shared utilities between stages.
    all_stages: String,
    stages: HashMap<ShaderStage, String>,
}

impl ProcessedShaderCode {
    pub fn stage(&self, stage: ShaderStage) -> Option<String> {
        self.stages
            .get(&stage)
            .map(|code| format!("{prefix}{code}", prefix = self.all_stages))
    }
}

fn emit_shader_code(state: &mut ShaderLoaderState, id: ShaderId) -> Result<ProcessedShaderCode> {
    let mut code = ProcessedShaderCode::default();
    let mut stage_stack: Vec<Option<ShaderStage>> = vec![None];

    visit_fragments_at_path(state, id, &mut |event| {
        // match event {
        //     ShaderParseEvent::Fragment(_) => log::debug!("got event: <fragment>"),
        //     other => log::debug!("got event: {other:?}"),
        // }
        match event {
            &ShaderParseEvent::Include(_) => unreachable!(),
            &ShaderParseEvent::Fragment(src) => {
                let stage = stage_stack
                    .iter()
                    .rfind(|stage| stage.is_some())
                    .and_then(|&stage| stage);

                match stage {
                    Some(stage) => code.stages.entry(stage).or_default().push_str(src),
                    None => code.all_stages.push_str(src),
                }
            }
            &ShaderParseEvent::Start => stage_stack.push(None),
            &ShaderParseEvent::End => drop(stage_stack.pop().unwrap()),
            &ShaderParseEvent::ShaderStage(stage) => *stage_stack.last_mut().unwrap() = Some(stage),
        }
    })?;

    Ok(code)
}

fn parse_shader<'src>(parser: &mut ShaderParser<'src, '_>) -> Result<Vec<ShaderParseEvent<'src>>> {
    add_shader_parse_event(parser, ShaderParseEvent::Start);

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

    add_shader_parse_event(parser, ShaderParseEvent::End);

    if !parser.errors.is_empty() {
        for error in parser.errors.iter() {
            log::error!(
                "error in shader '{}': {}",
                parser.norm_path.display(),
                error
            );
        }
        bail!(
            "{} errors encountered in shader '{}'",
            parser.errors.len(),
            parser.norm_path.display()
        );
    }

    Ok(std::mem::take(&mut parser.events))
}

fn load_shader_internal(state: &mut ShaderLoaderState, id: ShaderId) -> Result<()> {
    log::trace!(
        "loading shader {} ({})",
        id.0,
        state.id_to_norm_path[&id].display()
    );

    let real_path = state.base_path.join(&state.id_to_norm_path[&id]);
    state.info_mut(id).raw_source = Arc::new(std::fs::read_to_string(&real_path)?);
    let code = emit_shader_code(state, id)?;

    // ShaderStage::enumerate(|stage| {
    //     if let Some(src) = code.stage(stage) {
    //         log::debug!("emitted for stage {:?}: \n\n{}\n\n", stage, src);
    //     }
    // });

    if state.info_mut(id).compile {
        let vertex_shader = code.stage(ShaderStage::Vertex).ok_or_else(|| {
            anyhow!(
                "shader '{}' is missing a vertex stage",
                state.id_to_norm_path[&id].display()
            )
        })?;

        let fragment_shader = code.stage(ShaderStage::Fragment).ok_or_else(|| {
            anyhow!(
                "shader '{}' is missing a fragment stage",
                state.id_to_norm_path[&id].display()
            )
        })?;

        let tessellation_control_shader = code.stage(ShaderStage::TesselationControl);
        let tessellation_evaluation_shader = code.stage(ShaderStage::TesselationEvaluation);
        let geometry_shader = code.stage(ShaderStage::Geometry);

        let program = Program::new(&*state.display, SourceCode {
            vertex_shader: &vertex_shader,
            tessellation_control_shader: tessellation_control_shader.as_deref(),
            tessellation_evaluation_shader: tessellation_evaluation_shader.as_deref(),
            geometry_shader: geometry_shader.as_deref(),
            fragment_shader: &fragment_shader,
        });

        let program = Rc::new(match program {
            Ok(program) => program,
            Err(err) if state.info_mut(id).program.is_some() => {
                log::error!("shader reload failed: \n\n{}\n\n", err);
                return Ok(());
            }
            Err(err) => return Err(anyhow!(err)),
        });

        state.info_mut(id).program = Some(Rc::clone(&program));
    }

    Ok(())
}

fn collect_dirty_shaders(
    state: &ShaderLoaderState,
    dirty: &mut HashSet<ShaderId>,
    id: ShaderId,
) -> Result<()> {
    if let Some(info) = state.infos.get(&id) {
        dirty.insert(info.id);
        for &dependant in info.dependants.iter() {
            collect_dirty_shaders(state, dirty, dependant)?;
        }
    }
    Ok(())
}

fn notify_shader_modified(state: &mut ShaderLoaderState, abs_path: &Path) -> Result<()> {
    let norm_path = state.relativize_path(abs_path)?;
    if !state.norm_path_to_id.contains_key(norm_path) {
        return Ok(());
    }

    let id = state.id(norm_path)?;
    let mut dirty = HashSet::new();
    collect_dirty_shaders(state, &mut dirty, id)?;
    for &id in dirty.iter() {
        load_shader_internal(state, id)?;
    }

    Ok(())
}

#[cfg(feature = "hot-reload")]
#[derive(Debug, Default)]
pub struct HotReloadPlugin {}

#[cfg(feature = "hot-reload")]
pub struct FileWatcher {
    channel: ChannelPair<notify::Result<notify::Event>>,
    _watcher: notify::RecommendedWatcher,
}

#[cfg(feature = "hot-reload")]
impl Plugin for HotReloadPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.add_event::<notify::Event>();
        app.add_startup_system(util::try_system!(file_watcher_init));
        app.add_system(file_watcher.system());

        app.add_system_to_stage(
            RenderStage::BeginRender,
            util::try_system!(hot_reload_shaders),
        );
    }
}

#[cfg(feature = "hot-reload")]
pub fn file_watcher_init(mut cmd: Commands) -> Result<()> {
    use notify::{RecursiveMode, Watcher};

    let channel = util::ChannelPair::default();
    let sender = channel.sender();
    let mut watcher = notify::recommended_watcher(move |event| {
        if sender.send(event).is_err() {
            return;
        }
    })?;
    // FIXME: move somewhere appropriate
    watcher.watch(Path::new("resources/shaders"), RecursiveMode::Recursive)?;
    cmd.insert_resource(FileWatcher {
        channel,
        _watcher: watcher,
    });
    Ok(())
}

#[cfg(feature = "hot-reload")]
pub fn file_watcher(watcher: Res<FileWatcher>, mut watcher_events: EventWriter<notify::Event>) {
    for event in watcher.channel.rx.try_iter() {
        match event {
            Ok(event) => watcher_events.send(event),
            Err(err) => log::warn!("file watcher error: {}", err),
        }
    }
}

#[cfg(feature = "hot-reload")]
pub fn hot_reload_shaders(
    mut shaders: NonSendMut<ShaderLoaderState>,
    mut watcher_events: EventReader<notify::Event>,
) -> Result<()> {
    use notify::{event::ModifyKind, EventKind};

    for event in watcher_events.iter() {
        match &event.kind {
            EventKind::Create(_) => {}
            EventKind::Modify(kind) => match kind {
                // don't worry about renames for now
                ModifyKind::Name(_) => {}
                _ => {
                    for path in event.paths.iter() {
                        let abs_path = path.canonicalize()?;
                        if let Err(err) = notify_shader_modified(&mut shaders, &abs_path) {
                            log::error!("shader hot-reload failed: {}", err);
                        }
                    }
                }
            },
            _ => {}
        }
    }
    Ok(())
}
