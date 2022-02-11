use super::{super::camera::CurrentCamera, Tex};
use crate::{
    client::{
        camera::Camera,
        loader::{self, ShaderLoaderState},
        render::mesher::TerrainMesh,
    },
    total_float::TotalFloat,
};
use bevy_ecs::system::SystemParam;
use crossbeam_channel::{Receiver, Sender};
use glium::{
    backend::Facade,
    framebuffer::{
        ColorAttachment, DepthAttachment, DepthStencilAttachment, SimpleFrameBuffer,
        StencilAttachment, ToColorAttachment, ToDepthAttachment, ToDepthStencilAttachment,
        ToStencilAttachment,
    },
    index::{IndexBuffer, PrimitiveType},
    texture::*,
    uniform,
    uniforms::{AsUniformValue, MagnifySamplerFilter, Sampler, UniformValue},
    vertex::VertexBuffer,
    Blend, Display, DrawParameters, Frame, Surface,
};
use notcraft_common::{
    aabb::Aabb, math::*, prelude::*, transform::Transform, util, world::registry::BlockRegistry,
};
use parking_lot::RwLock;
use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
    path::PathBuf,
    rc::Rc,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

struct RendererMisc {
    fullscreen_quad: VertexBuffer<Tex>,
    // crosshair_quad: VertexBuffer<PosTex>,
    // FIXME: this shouldn't be here! make a more general static texture loader thingy when this
    // becomes a problem
    block_textures: SrgbTexture2dArray,
    crosshair_texture: SrgbTexture2d,
}

impl RendererMisc {
    pub fn new(display: &Rc<Display>, registry: &Arc<BlockRegistry>) -> Result<Self> {
        let fullscreen_quad = VertexBuffer::immutable(&**display, &[
            Tex { uv: [-1.0, 1.0] },
            Tex { uv: [1.0, 1.0] },
            Tex { uv: [-1.0, -1.0] },
            Tex { uv: [1.0, 1.0] },
            Tex { uv: [-1.0, -1.0] },
            Tex { uv: [1.0, -1.0] },
        ])?;

        // #[rustfmt::skip]
        // const CROSSHAIR_QUAD_DATA: &[PosTex] = &[
        //     PosTex { pos: [-0.1,  0.1, 0.0], uv: [-1.0,  1.0] },
        //     PosTex { pos: [ 0.1,  0.1, 0.0], uv: [ 1.0,  1.0] },
        //     PosTex { pos: [-0.1, -0.1, 0.0], uv: [-1.0, -1.0] },
        //     PosTex { pos: [ 0.1,  0.1, 0.0], uv: [ 1.0,  1.0] },
        //     PosTex { pos: [-0.1, -0.1, 0.0], uv: [-1.0, -1.0] },
        //     PosTex { pos: [ 0.1, -0.1, 0.0], uv: [ 1.0, -1.0] },
        // ];
        // let crosshair_quad = VertexBuffer::immutable(&**display,
        // CROSSHAIR_QUAD_DATA)?;

        let crosshair_texture = loader::load_texture("resources/textures/crosshair.png")?;
        let crosshair_texture = SrgbTexture2d::new(
            &**display,
            RawImage2d::from_raw_rgba_reversed(&crosshair_texture, crosshair_texture.dimensions()),
        )?;

        let textures =
            loader::load_block_textures("resources/textures/blocks", registry.texture_paths())?;

        let textures = registry
            .texture_paths()
            .map(|name| {
                let map = &textures.block_textures[name];
                RawImage2d::from_raw_rgba_reversed(map, map.dimensions())
            })
            .collect();

        let block_textures =
            SrgbTexture2dArray::with_mipmaps(&**display, textures, MipmapsOption::NoMipmap)?;

        Ok(Self {
            fullscreen_quad,
            // crosshair_quad,
            block_textures,
            crosshair_texture,
        })
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, StageLabel)]
pub enum RenderStage {
    BeginRender,
    PreRender,
    Render,
    PostRender,
    EndRender,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, SystemLabel)]
pub struct RenderLabel<T>(pub T);

#[derive(Debug, Default)]
pub struct RenderPlugin {}

impl Plugin for RenderPlugin {
    // my god this is awful
    fn build(&self, app: &mut AppBuilder) {
        // very unfortunate limitation of `Plugin`s, they require a `Send + Sync +
        // 'static` bound
        let display = app
            .world()
            .get_non_send_resource::<Rc<Display>>()
            .cloned()
            .expect(
                "`RenderPlugin` added before `WindowingPlugin`! (no `Rc<Display>` resource exists)",
            );

        // FIXME: i dont like this
        let registry = app
            .world()
            .get_non_send_resource::<Arc<BlockRegistry>>()
            .cloned()
            .expect(
                "`RenderPlugin` added before `WorldPlugin`! (no `BlockRegistry` resource exists)",
            );

        app.add_startup_system(util::try_system!(declare_targets));

        app.insert_non_send_resource(RenderTargets::new(&display));
        app.insert_non_send_resource(
            // FIXME: * e r r o r   h a n d l i n g *
            ShaderLoaderState::load(&display, PathBuf::from("resources/shaders")).unwrap(),
        );
        app.insert_non_send_resource(DebugLines::new());
        app.insert_non_send_resource(RendererMisc::new(&display, &registry).unwrap());

        // mesh context
        let local = LocalMeshContext::<TerrainMesh>::new();
        app.insert_resource(Arc::clone(&local.shared));
        app.insert_non_send_resource(local);

        app.init_resource::<ImmediateLines>();

        app.add_stage_after(
            CoreStage::PostUpdate,
            RenderStage::Render,
            SystemStage::single_threaded(),
        )
        .add_stage_before(
            RenderStage::Render,
            RenderStage::PreRender,
            SystemStage::single_threaded(),
        )
        .add_stage_before(
            RenderStage::PreRender,
            RenderStage::BeginRender,
            SystemStage::single_threaded(),
        )
        .add_stage_after(
            RenderStage::Render,
            RenderStage::PostRender,
            SystemStage::single_threaded(),
        )
        .add_stage_after(
            RenderStage::PostRender,
            RenderStage::EndRender,
            SystemStage::single_threaded(),
        );

        app.add_system_to_stage(
            RenderStage::Render,
            util::try_system!(render_sky)
                .label(RenderLabel("sky"))
                .label(RenderLabel("world")),
        )
        .add_system_to_stage(
            RenderStage::Render,
            util::try_system!(render_post)
                .label(RenderLabel("post"))
                .after(RenderLabel("world")),
        )
        .add_system_to_stage(
            RenderStage::Render,
            util::try_system!(render_terrain)
                .label(RenderLabel("world"))
                .label(RenderLabel("terrain"))
                .after(RenderLabel("sky")),
        )
        .add_system_to_stage(
            RenderStage::Render,
            add_global_debug_lines
                .system()
                .label(RenderLabel("add_global_debug_lines")),
        )
        .add_system_to_stage(
            RenderStage::Render,
            util::try_system!(render_lines)
                .label(RenderLabel("world"))
                .after(RenderLabel("terrain"))
                .after(RenderLabel("add_global_debug_lines")),
        );
        app.add_system_to_stage(RenderStage::BeginRender, util::try_system!(begin_render));
        app.add_system_to_stage(RenderStage::EndRender, util::try_system!(end_render));
    }
}

pub struct RenderTargets {
    display: Rc<Display>,
    descriptors: HashMap<String, RenderTargetDesc>,
    targets: HashMap<String, ((u32, u32), RenderTarget)>,
    previous_size: (u32, u32),
    frame: Option<Frame>,
}

pub enum RenderTarget {
    Color {
        color: RenderTargetTexture,
    },
    Depth {
        depth: RenderTargetTexture,
    },
    ColorDepth {
        color: RenderTargetTexture,
        depth: RenderTargetTexture,
    },
}

#[derive(Copy, Clone, Debug)]
pub struct RenderTargetDesc {
    pub size: RenderTargetSize,
    pub kind: RenderTargetKind,
    pub samples: Option<u32>,
}

#[derive(Copy, Clone, Debug)]
pub enum RenderTargetSize {
    WindowExact,
    WindowScaledDown(u32),
    WindowScaledUp(u32),
    Exact(u32, u32),
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RenderTargetKind {
    ColorOnly {
        color: ColorTextureFormat,
        clear_color: Option<[f32; 4]>,
    },
    DepthOnly {
        depth: DepthStencilTextureFormat,
        clear_depth: Option<f32>,
    },
    ColorDepth {
        color: ColorTextureFormat,
        depth: DepthStencilTextureFormat,
        clear_color: Option<[f32; 4]>,
        clear_depth: Option<f32>,
    },
}

#[derive(Debug)]
pub enum RenderTargetTexture {
    Float(Texture2d),
    Integral(IntegralTexture2d),
    Unsigned(UnsignedTexture2d),
    Srgb(SrgbTexture2d),
    Depth(DepthTexture2d),
    Stencil(StencilTexture2d),
    DepthStencil(DepthStencilTexture2d),
    FloatMulti(Texture2dMultisample),
    IntegralMulti(IntegralTexture2dMultisample),
    UnsignedMulti(UnsignedTexture2dMultisample),
    SrgbMulti(SrgbTexture2dMultisample),
    DepthMulti(DepthTexture2dMultisample),
    StencilMulti(StencilTexture2dMultisample),
    DepthStencilMulti(DepthStencilTexture2dMultisample),
}

pub enum RenderTargetTextureUniform<'a> {
    Float(Sampler<'a, Texture2d>),
    Integral(Sampler<'a, IntegralTexture2d>),
    Unsigned(Sampler<'a, UnsignedTexture2d>),
    Srgb(Sampler<'a, SrgbTexture2d>),
    Depth(Sampler<'a, DepthTexture2d>),
    FloatMulti(Sampler<'a, Texture2dMultisample>),
    IntegralMulti(Sampler<'a, IntegralTexture2dMultisample>),
    UnsignedMulti(Sampler<'a, UnsignedTexture2dMultisample>),
    SrgbMulti(Sampler<'a, SrgbTexture2dMultisample>),
    DepthMulti(Sampler<'a, DepthTexture2dMultisample>),
}

impl<'a> RenderTargetTextureUniform<'a> {
    pub fn magnify_filter(self, filter: MagnifySamplerFilter) -> Self {
        match self {
            Self::Float(sampler) => Self::Float(sampler.magnify_filter(filter)),
            Self::Integral(sampler) => Self::Integral(sampler.magnify_filter(filter)),
            Self::Unsigned(sampler) => Self::Unsigned(sampler.magnify_filter(filter)),
            Self::Srgb(sampler) => Self::Srgb(sampler.magnify_filter(filter)),
            Self::Depth(sampler) => Self::Depth(sampler.magnify_filter(filter)),
            Self::FloatMulti(sampler) => Self::FloatMulti(sampler.magnify_filter(filter)),
            Self::IntegralMulti(sampler) => Self::IntegralMulti(sampler.magnify_filter(filter)),
            Self::UnsignedMulti(sampler) => Self::UnsignedMulti(sampler.magnify_filter(filter)),
            Self::SrgbMulti(sampler) => Self::SrgbMulti(sampler.magnify_filter(filter)),
            Self::DepthMulti(sampler) => Self::DepthMulti(sampler.magnify_filter(filter)),
        }
    }

    pub fn anisotropy(self, anisotropy: u16) -> Self {
        match self {
            Self::Float(sampler) => Self::Float(sampler.anisotropy(anisotropy)),
            Self::Integral(sampler) => Self::Integral(sampler.anisotropy(anisotropy)),
            Self::Unsigned(sampler) => Self::Unsigned(sampler.anisotropy(anisotropy)),
            Self::Srgb(sampler) => Self::Srgb(sampler.anisotropy(anisotropy)),
            Self::Depth(sampler) => Self::Depth(sampler.anisotropy(anisotropy)),
            Self::FloatMulti(sampler) => Self::FloatMulti(sampler.anisotropy(anisotropy)),
            Self::IntegralMulti(sampler) => Self::IntegralMulti(sampler.anisotropy(anisotropy)),
            Self::UnsignedMulti(sampler) => Self::UnsignedMulti(sampler.anisotropy(anisotropy)),
            Self::SrgbMulti(sampler) => Self::SrgbMulti(sampler.anisotropy(anisotropy)),
            Self::DepthMulti(sampler) => Self::DepthMulti(sampler.anisotropy(anisotropy)),
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ColorTextureFormat {
    UncompressedFloat(UncompressedFloatFormat),
    UncompressedIntegral(UncompressedIntFormat),
    UncompressedUnsigned(UncompressedUintFormat),
    Srgb(SrgbFormat),
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum DepthStencilTextureFormat {
    DepthFormat(DepthFormat),
    StencilFormat(StencilFormat),
    DepthStencilFormat(DepthStencilFormat),
}

impl RenderTargetSize {
    pub fn apply(&self, (width, height): (u32, u32)) -> (u32, u32) {
        match self {
            &RenderTargetSize::WindowExact => (width, height),
            &RenderTargetSize::WindowScaledDown(factor) => (width / factor, height / factor),
            &RenderTargetSize::WindowScaledUp(factor) => (width * factor, height * factor),
            &RenderTargetSize::Exact(width, height) => (width, height),
        }
    }
}

impl RenderTargetKind {
    pub fn clear_color(&self) -> Option<[f32; 4]> {
        match self {
            &RenderTargetKind::ColorOnly { clear_color, .. } => clear_color,
            &RenderTargetKind::DepthOnly { .. } => None,
            &RenderTargetKind::ColorDepth { clear_color, .. } => clear_color,
        }
    }

    pub fn clear_depth(&self) -> Option<f32> {
        match self {
            &RenderTargetKind::ColorOnly { .. } => None,
            &RenderTargetKind::DepthOnly { clear_depth, .. } => clear_depth,
            &RenderTargetKind::ColorDepth { clear_depth, .. } => clear_depth,
        }
    }
}

impl<'a> AsUniformValue for RenderTargetTextureUniform<'a> {
    fn as_uniform_value(&self) -> UniformValue<'_> {
        match self {
            RenderTargetTextureUniform::Float(texture) => texture.as_uniform_value(),
            RenderTargetTextureUniform::Integral(texture) => texture.as_uniform_value(),
            RenderTargetTextureUniform::Unsigned(texture) => texture.as_uniform_value(),
            RenderTargetTextureUniform::Srgb(texture) => texture.as_uniform_value(),
            RenderTargetTextureUniform::Depth(texture) => texture.as_uniform_value(),
            RenderTargetTextureUniform::FloatMulti(texture) => texture.as_uniform_value(),
            RenderTargetTextureUniform::IntegralMulti(texture) => texture.as_uniform_value(),
            RenderTargetTextureUniform::UnsignedMulti(texture) => texture.as_uniform_value(),
            RenderTargetTextureUniform::SrgbMulti(texture) => texture.as_uniform_value(),
            RenderTargetTextureUniform::DepthMulti(texture) => texture.as_uniform_value(),
        }
    }
}

impl RenderTargetTexture {
    pub fn uniform(&self) -> Result<RenderTargetTextureUniform<'_>> {
        Ok(match self {
            RenderTargetTexture::Float(texture) => {
                RenderTargetTextureUniform::Float(texture.sampled())
            }
            RenderTargetTexture::Integral(texture) => {
                RenderTargetTextureUniform::Integral(texture.sampled())
            }
            RenderTargetTexture::Unsigned(texture) => {
                RenderTargetTextureUniform::Unsigned(texture.sampled())
            }
            RenderTargetTexture::Srgb(texture) => {
                RenderTargetTextureUniform::Srgb(texture.sampled())
            }
            RenderTargetTexture::Depth(texture) => {
                RenderTargetTextureUniform::Depth(texture.sampled())
            }
            RenderTargetTexture::FloatMulti(texture) => {
                RenderTargetTextureUniform::FloatMulti(texture.sampled())
            }
            RenderTargetTexture::IntegralMulti(texture) => {
                RenderTargetTextureUniform::IntegralMulti(texture.sampled())
            }
            RenderTargetTexture::UnsignedMulti(texture) => {
                RenderTargetTextureUniform::UnsignedMulti(texture.sampled())
            }
            RenderTargetTexture::SrgbMulti(texture) => {
                RenderTargetTextureUniform::SrgbMulti(texture.sampled())
            }
            RenderTargetTexture::DepthMulti(texture) => {
                RenderTargetTextureUniform::DepthMulti(texture.sampled())
            }
            _ => anyhow::bail!("invalid uniform value: {:?}", self),
        })
    }
}

impl AsRef<TextureAny> for RenderTargetTexture {
    fn as_ref(&self) -> &TextureAny {
        match self {
            RenderTargetTexture::Float(texture) => &**texture,
            RenderTargetTexture::Integral(texture) => &**texture,
            RenderTargetTexture::Unsigned(texture) => &**texture,
            RenderTargetTexture::Srgb(texture) => &**texture,
            RenderTargetTexture::Depth(texture) => &**texture,
            RenderTargetTexture::Stencil(texture) => &**texture,
            RenderTargetTexture::DepthStencil(texture) => &**texture,
            RenderTargetTexture::FloatMulti(texture) => &**texture,
            RenderTargetTexture::IntegralMulti(texture) => &**texture,
            RenderTargetTexture::UnsignedMulti(texture) => &**texture,
            RenderTargetTexture::SrgbMulti(texture) => &**texture,
            RenderTargetTexture::DepthMulti(texture) => &**texture,
            RenderTargetTexture::StencilMulti(texture) => &**texture,
            RenderTargetTexture::DepthStencilMulti(texture) => &**texture,
        }
    }
}

impl RenderTargetTexture {
    pub fn as_color_attachment<'a>(&'a self) -> Result<ColorAttachment<'a>> {
        Ok(match self {
            RenderTargetTexture::Float(texture) => texture.to_color_attachment(),
            RenderTargetTexture::Integral(texture) => texture.to_color_attachment(),
            RenderTargetTexture::Unsigned(texture) => texture.to_color_attachment(),
            RenderTargetTexture::Srgb(texture) => texture.to_color_attachment(),
            RenderTargetTexture::FloatMulti(texture) => texture.to_color_attachment(),
            RenderTargetTexture::IntegralMulti(texture) => texture.to_color_attachment(),
            RenderTargetTexture::UnsignedMulti(texture) => texture.to_color_attachment(),
            RenderTargetTexture::SrgbMulti(texture) => texture.to_color_attachment(),
            _ => anyhow::bail!("invalid color attachment: {:?}", self),
        })
    }

    pub fn as_depth_attachment<'a>(&'a self) -> Result<DepthAttachment<'a>> {
        Ok(match self {
            RenderTargetTexture::Depth(texture) => texture.to_depth_attachment(),
            RenderTargetTexture::DepthMulti(texture) => texture.to_depth_attachment(),
            _ => anyhow::bail!("invalid depth attachment: {:?}", self),
        })
    }

    pub fn as_stencil_attachment<'a>(&'a self) -> Result<StencilAttachment<'a>> {
        Ok(match self {
            RenderTargetTexture::Stencil(texture) => texture.to_stencil_attachment(),
            RenderTargetTexture::StencilMulti(texture) => texture.to_stencil_attachment(),
            _ => anyhow::bail!("invalid stencil_attachment: {:?}", self),
        })
    }

    pub fn as_depth_stencil_attachment<'a>(&'a self) -> Result<DepthStencilAttachment<'a>> {
        Ok(match self {
            RenderTargetTexture::DepthStencil(texture) => texture.to_depth_stencil_attachment(),
            RenderTargetTexture::DepthStencilMulti(texture) => {
                texture.to_depth_stencil_attachment()
            }
            _ => anyhow::bail!("invalid depth-stencil attachment: {:?}", self),
        })
    }
}

impl RenderTarget {
    fn framebuffer<'t>(&'t self, display: &Display) -> Result<SimpleFrameBuffer<'t>> {
        match self {
            RenderTarget::Color { color } => {
                let color = color.as_color_attachment()?;
                Ok(SimpleFrameBuffer::new(display, color)?)
            }
            RenderTarget::Depth { depth } => {
                let depth = depth.as_depth_attachment()?;
                Ok(SimpleFrameBuffer::depth_only(display, depth)?)
            }
            RenderTarget::ColorDepth { color, depth } => {
                let color = color.as_color_attachment()?;
                let depth = depth.as_depth_attachment()?;
                Ok(SimpleFrameBuffer::with_depth_buffer(display, color, depth)?)
            }
        }
    }

    pub fn depth(&self) -> Option<&RenderTargetTexture> {
        match self {
            RenderTarget::Depth { depth } => Some(depth),
            RenderTarget::ColorDepth { depth, .. } => Some(depth),
            _ => None,
        }
    }

    pub fn color(&self) -> Option<&RenderTargetTexture> {
        match self {
            RenderTarget::Color { color } => Some(color),
            RenderTarget::ColorDepth { color, .. } => Some(color),
            _ => None,
        }
    }
}

fn make_texture_from_desc(ctx: &Display, desc: RenderTargetDesc) -> anyhow::Result<RenderTarget> {
    let (width, height) = desc.size.apply(ctx.get_framebuffer_dimensions());
    let (color, depth) = match desc.kind {
        RenderTargetKind::ColorOnly { color, .. } => (Some(color), None),
        RenderTargetKind::DepthOnly { depth, .. } => (None, Some(depth)),
        RenderTargetKind::ColorDepth { color, depth, .. } => (Some(color), Some(depth)),
    };

    macro_rules! make_texture {
        ($kind:ident($tex:ident), $kind_multi:ident($tex_multi:ident), $format:expr) => {{
            use RenderTargetTexture::*;
            Some(match desc.samples {
                Some(samples) => $kind_multi($tex_multi::empty_with_format(
                    ctx,
                    $format,
                    MipmapsOption::NoMipmap,
                    width,
                    height,
                    samples,
                )?),
                None => $kind($tex::empty_with_format(
                    ctx,
                    $format,
                    MipmapsOption::NoMipmap,
                    width,
                    height,
                )?),
            })
        }};
    }

    use ColorTextureFormat::*;
    let color = match color {
        Some(UncompressedFloat(format)) => {
            make_texture!(Float(Texture2d), FloatMulti(Texture2dMultisample), format)
        }
        Some(UncompressedIntegral(format)) => {
            make_texture!(
                Integral(IntegralTexture2d),
                IntegralMulti(IntegralTexture2dMultisample),
                format
            )
        }
        Some(UncompressedUnsigned(format)) => {
            make_texture!(
                Unsigned(UnsignedTexture2d),
                UnsignedMulti(UnsignedTexture2dMultisample),
                format
            )
        }
        Some(Srgb(format)) => {
            make_texture!(
                Srgb(SrgbTexture2d),
                SrgbMulti(SrgbTexture2dMultisample),
                format
            )
        }
        None => None,
    };

    let depth = match depth {
        Some(DepthStencilTextureFormat::DepthFormat(format)) => {
            make_texture!(
                Depth(DepthTexture2d),
                DepthMulti(DepthTexture2dMultisample),
                format
            )
        }
        Some(DepthStencilTextureFormat::StencilFormat(format)) => {
            make_texture!(
                Stencil(StencilTexture2d),
                StencilMulti(StencilTexture2dMultisample),
                format
            )
        }
        Some(DepthStencilTextureFormat::DepthStencilFormat(format)) => {
            make_texture!(
                DepthStencil(DepthStencilTexture2d),
                DepthStencilMulti(DepthStencilTexture2dMultisample),
                format
            )
        }
        None => None,
    };

    Ok(match desc.kind {
        RenderTargetKind::ColorOnly { .. } => RenderTarget::Color {
            color: color.unwrap(),
        },
        RenderTargetKind::DepthOnly { .. } => RenderTarget::Depth {
            depth: depth.unwrap(),
        },
        RenderTargetKind::ColorDepth { .. } => RenderTarget::ColorDepth {
            color: color.unwrap(),
            depth: depth.unwrap(),
        },
    })
}

impl RenderTargets {
    fn new(display: &Rc<Display>) -> Self {
        Self {
            display: Rc::clone(display),
            descriptors: Default::default(),
            targets: Default::default(),
            previous_size: display.get_framebuffer_dimensions(),
            frame: None,
        }
    }

    pub fn declare_target(&mut self, name: &str, desc: RenderTargetDesc) -> anyhow::Result<()> {
        let dimensions = desc.size.apply(self.display.get_framebuffer_dimensions());
        self.descriptors.insert(name.into(), desc);
        self.targets.insert(
            name.into(),
            (dimensions, make_texture_from_desc(&self.display, desc)?),
        );
        Ok(())
    }

    pub fn declare_resolve_target(&mut self, name: &str, source: &str) -> anyhow::Result<()> {
        let desc = RenderTargetDesc {
            samples: None,
            ..*self.descriptors.get(source).unwrap()
        };
        self.declare_target(name, desc)
    }

    pub fn get(&self, name: &str) -> Result<&RenderTarget> {
        self.targets
            .get(name)
            .map(|(_, x)| x)
            .ok_or_else(|| anyhow::anyhow!("render target '{}' was not registered", name))
    }

    pub fn resize(&mut self, dimensions: (u32, u32)) -> anyhow::Result<()> {
        for (name, &desc) in self.descriptors.iter() {
            let (old_dims, buffer) = self.targets.get_mut(name).unwrap();
            let new_dims = desc.size.apply(dimensions);
            if *old_dims != new_dims {
                *buffer = make_texture_from_desc(&self.display, desc)?;
                *old_dims = new_dims;
            }
        }
        Ok(())
    }

    // clear all textures that have specified clear values.
    pub fn reset(&mut self) -> Result<()> {
        let new_dims = self.display.get_framebuffer_dimensions();
        if self.previous_size != new_dims {
            self.resize(new_dims)?;
        }
        for (name, &desc) in self.descriptors.iter() {
            let (_, buffer) = self.targets.get_mut(name).unwrap();
            if let Some([r, g, b, a]) = desc.kind.clear_color() {
                buffer.framebuffer(&self.display)?.clear_color(r, g, b, a);
            }
            if let Some(depth) = desc.kind.clear_depth() {
                buffer.framebuffer(&self.display)?.clear_depth(depth);
            }
        }

        Ok(())
    }
}

fn declare_targets(mut targets: NonSendMut<RenderTargets>) -> Result<()> {
    targets.declare_target("world", RenderTargetDesc {
        size: RenderTargetSize::WindowExact,
        kind: RenderTargetKind::ColorDepth {
            color: ColorTextureFormat::UncompressedFloat(UncompressedFloatFormat::F16F16F16),
            depth: DepthStencilTextureFormat::DepthFormat(DepthFormat::F32),
            clear_color: None, // completely filled in with sky pass
            clear_depth: Some(1.0),
        },
        samples: None,
    })?;

    targets.declare_target("final", RenderTargetDesc {
        size: RenderTargetSize::WindowExact,
        kind: RenderTargetKind::ColorOnly {
            color: ColorTextureFormat::UncompressedFloat(UncompressedFloatFormat::F16F16F16),
            clear_color: None, // completely filled in with post pass
        },
        samples: None,
    })?;

    Ok(())
}

#[derive(Debug)]
pub struct MeshBuffers<V: Copy> {
    pub vertices: VertexBuffer<V>,
    pub indices: IndexBuffer<u32>,
    // mesh bounds, in model space
    pub aabb: Aabb,
}

#[derive(Debug)]
pub struct MeshHandle<M>(Arc<MeshHandleInner<M>>);

// unsafe impl<M> Send for MeshHandle<M> {}
// unsafe impl<M> Sync for MeshHandle<M> {}

impl<M> Clone for MeshHandle<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<M> MeshHandle<M> {
    pub fn reupload(&self, mesh: M) {
        self.0.shared.mesh_sender.send((self.0.id, mesh)).unwrap();
    }
}

#[derive(Debug)]
pub struct MeshHandleInner<M> {
    id: usize,
    shared: Arc<SharedMeshContext<M>>,
    _phantom: PhantomData<M>,
}

impl<M> Drop for MeshHandleInner<M> {
    fn drop(&mut self) {
        // should be ok to ignore the result here, if the render thread shut down, then
        // that means the meshes were all already dropped.
        let _ = self.shared.mesh_dropped_sender.send(self.id);
    }
}

pub trait UploadableMesh {
    type Vertex: Copy;

    fn upload<F: Facade>(&self, ctx: &F) -> Result<MeshBuffers<Self::Vertex>>;
}

struct LocalMeshContext<M: UploadableMesh> {
    shared: Arc<SharedMeshContext<M>>,
    meshes: HashMap<usize, MeshBuffers<M::Vertex>>,
}

impl<M: UploadableMesh + Send + Sync + 'static> LocalMeshContext<M> {
    pub fn new() -> Self {
        Self {
            shared: SharedMeshContext::new(),
            meshes: Default::default(),
        }
    }

    fn update<F: Facade>(&mut self, ctx: &F) -> Result<()> {
        for (id, data) in self.shared.mesh_receiver.try_iter() {
            self.meshes.insert(id, data.upload(ctx)?);
        }

        for id in self.shared.mesh_dropped_receiver.try_iter() {
            self.meshes.remove(&id);
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct SharedMeshContext<M> {
    next_id: AtomicUsize,
    mesh_receiver: Receiver<(usize, M)>,
    mesh_sender: Sender<(usize, M)>,
    mesh_dropped_receiver: Receiver<usize>,
    mesh_dropped_sender: Sender<usize>,
}

impl<M> SharedMeshContext<M> {
    pub fn new() -> Arc<SharedMeshContext<M>> {
        let (mesh_sender, mesh_receiver) = crossbeam_channel::unbounded();
        let (mesh_dropped_sender, mesh_dropped_receiver) = crossbeam_channel::unbounded();

        Arc::new(Self {
            next_id: AtomicUsize::new(0),
            mesh_receiver,
            mesh_sender,
            mesh_dropped_receiver,
            mesh_dropped_sender,
        })
    }

    pub fn upload(self: &Arc<Self>, mesh: M) -> MeshHandle<M> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.mesh_sender.send((id, mesh)).unwrap();
        MeshHandle(Arc::new(MeshHandleInner {
            id,
            shared: Arc::clone(&self),
            _phantom: PhantomData,
        }))
    }
}

#[derive(Debug)]
pub struct RenderMeshComponent<M>(MeshHandle<M>);

impl<M> RenderMeshComponent<M> {
    pub fn new(handle: MeshHandle<M>) -> Self {
        Self(handle)
    }
}

#[derive(SystemParam)]
pub struct RenderParams<'a> {
    display: NonSend<'a, Rc<Display>>,
    pub targets: NonSendMut<'a, RenderTargets>,
    pub shaders: NonSendMut<'a, ShaderLoaderState>,
}

impl<'a> RenderParams<'a> {
    /// Get a reference to the render params's display.
    pub fn display(&self) -> &Display {
        &**self.display
    }
}

fn should_draw_aabb(mvp: &Matrix4<f32>, aabb: &Aabb) -> bool {
    // an AABB is excluded from the test if all its 8 corners lay outside any single
    // frustum plane. we transform into clip space because the camera frustum planes
    // have some very nice properties. each plane is 1 unit from the origin along
    // its respective axis, and points inwards directly towards the origin. because
    // of this, the test for e.x. the bottom plane is simply `point.y / point.w >
    // -1.0`. we can just test `point.y > -point.w` though, by multiplying both
    // sides of the inequality by `point.w`

    // my first attempt at this only tested if each corner was inside the camera
    // frustum, instead of outside any frustum plane, which led to some false
    // negatives where the corners would straddle the corner of the frustum, so the
    // line connecting them would cross through the frustum. this means that the
    // object might potentially influence the resulting image, but was excluded
    // because those points weren't actually inside the frustum.

    let corners_clip = [
        mvp * point![aabb.min.x, aabb.min.y, aabb.min.z, 1.0],
        mvp * point![aabb.max.x, aabb.min.y, aabb.min.z, 1.0],
        mvp * point![aabb.min.x, aabb.max.y, aabb.min.z, 1.0],
        mvp * point![aabb.max.x, aabb.max.y, aabb.min.z, 1.0],
        mvp * point![aabb.min.x, aabb.min.y, aabb.max.z, 1.0],
        mvp * point![aabb.max.x, aabb.min.y, aabb.max.z, 1.0],
        mvp * point![aabb.min.x, aabb.max.y, aabb.max.z, 1.0],
        mvp * point![aabb.max.x, aabb.max.y, aabb.max.z, 1.0],
    ];

    let px = !corners_clip.iter().all(|point| point.x > point.w);
    let nx = !corners_clip.iter().all(|point| point.x < -point.w);
    let py = !corners_clip.iter().all(|point| point.y > point.w);
    let ny = !corners_clip.iter().all(|point| point.y < -point.w);
    let pz = !corners_clip.iter().all(|point| point.z > point.w);
    let nz = !corners_clip.iter().all(|point| point.z < -point.w);

    px && nx && py && ny && pz && nz
}

pub fn array4x4<T: Copy + Into<[[U; 4]; 4]>, U>(mat: &T) -> [[U; 4]; 4] {
    (*mat).into()
}

pub fn array2<T: Copy + Into<[U; 2]>, U>(vec: &T) -> [U; 2] {
    (*vec).into()
}

pub fn array3<T: Copy + Into<[U; 3]>, U>(vec: &T) -> [U; 3] {
    (*vec).into()
}

lazy_static::lazy_static! {
    static ref DEBUG_BOX_SENDER: RwLock<Option<Sender<DebugBox>>> = RwLock::new(None);
    static ref TRANSIENT_DEBUG_BOX_SENDER: RwLock<Option<Sender<(Duration, DebugBox)>>> = RwLock::new(None);
}

pub fn add_debug_box(debug_box: DebugBox) {
    if let Some(sender) = DEBUG_BOX_SENDER.read().as_ref() {
        sender.send(debug_box).unwrap();
    }
}

pub fn add_transient_debug_box(duration: Duration, debug_box: DebugBox) {
    if let Some(sender) = TRANSIENT_DEBUG_BOX_SENDER.read().as_ref() {
        sender.send((duration, debug_box)).unwrap();
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum DebugBoxKind {
    Solid = 0,
    Dashed = 1,
    Dotted = 2,
}

#[derive(Copy, Clone, Debug)]
pub struct DebugBox {
    pub bounds: Aabb,
    pub rgba: [f32; 4],
    pub width: f32,
    pub kind: DebugBoxKind,
}

impl DebugBox {
    pub fn new(bounds: Aabb) -> Self {
        Self {
            bounds,
            rgba: [1.0; 4],
            width: 1.0,
            kind: DebugBoxKind::Solid,
        }
    }

    pub fn with_color<C: Into<[f32; 4]>>(mut self, color: C) -> Self {
        self.rgba = color.into();
        self
    }

    pub fn with_width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn with_kind(mut self, kind: DebugBoxKind) -> Self {
        self.kind = kind;
        self
    }
}

struct DebugLines {
    debug_box_channel: util::ChannelPair<DebugBox>,
    transient_debug_box_channel: util::ChannelPair<(Duration, DebugBox)>,
    next_transient_id: usize,
    transient_debug_boxes: HashMap<usize, (Instant, Duration, DebugBox)>,
    dead_transient_debug_boxes: HashSet<usize>,
}

impl DebugLines {
    fn new() -> Self {
        let debug_box_channel = util::ChannelPair::default();
        let transient_debug_box_channel = util::ChannelPair::default();

        *DEBUG_BOX_SENDER.write() = Some(debug_box_channel.sender());
        *TRANSIENT_DEBUG_BOX_SENDER.write() = Some(transient_debug_box_channel.sender());

        Self {
            debug_box_channel,
            transient_debug_box_channel,
            next_transient_id: 0,
            transient_debug_boxes: Default::default(),
            dead_transient_debug_boxes: Default::default(),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
#[repr(C)]
pub struct ImmediateLineVertex {
    pub pos: [f32; 3],
    pub color_rg: u32,
    pub color_ba: u32,
}
glium::implement_vertex!(ImmediateLineVertex, pos, color_rg, color_ba);

fn translate_color(color: [f32; 4]) -> [u32; 2] {
    let [r, g, b, a] = color.map(|comp| (util::clamp(0.0, 1.0, comp) * u16::MAX as f32) as u32);
    [(r << 16) | g, (b << 16) | a]
}

#[derive(Clone, Debug, Default)]
pub struct ImmediateLines {
    vertices: Vec<ImmediateLineVertex>,
    lines: HashMap<TotalFloat<f32>, Vec<u32>>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LineCanvasParams {
    pub width: f32,
    pub color: [f32; 4],
}

impl Default for LineCanvasParams {
    fn default() -> Self {
        Self {
            width: 1.0,
            color: [1.0; 4],
        }
    }
}

pub struct LineCanvas<'lines> {
    lines: &'lines mut ImmediateLines,
    current_color_rg: u32,
    current_color_ba: u32,
    current_width: TotalFloat<f32>,
    last_pos: [f32; 3],
}

impl<'lines> LineCanvas<'lines> {
    pub fn goto<I: Into<[f32; 3]>>(&mut self, pos: I) -> &mut Self {
        self.last_pos = pos.into();
        self.lines.vertices.push(ImmediateLineVertex {
            pos: self.last_pos,
            color_rg: self.current_color_rg,
            color_ba: self.current_color_ba,
        });
        self
    }

    pub fn line<I: Into<[f32; 3]>>(&mut self, pos: I) -> &mut Self {
        let entry = self.lines.lines.entry(self.current_width).or_default();

        self.last_pos = pos.into();
        entry.push(self.lines.vertices.len() as u32 - 1);
        entry.push(self.lines.vertices.len() as u32);
        self.lines.vertices.push(ImmediateLineVertex {
            pos: self.last_pos,
            color_rg: self.current_color_rg,
            color_ba: self.current_color_ba,
        });
        self
    }

    pub fn color<I: Into<[f32; 4]>>(&mut self, color: I) -> &mut Self {
        let [rg, ba] = translate_color(color.into());
        self.current_color_rg = rg;
        self.current_color_ba = ba;
        self
    }

    pub fn width(&mut self, width: f32) -> &mut Self {
        self.current_width = TotalFloat(width);
        self
    }
}

impl ImmediateLines {
    pub fn start_default(&mut self) -> LineCanvas {
        self.start([0.0; 3], Default::default())
    }

    pub fn start<I: Into<[f32; 3]>>(&mut self, pos: I, params: LineCanvasParams) -> LineCanvas {
        let [rg, ba] = translate_color(params.color);
        LineCanvas {
            lines: self,
            current_color_rg: rg,
            current_color_ba: ba,
            current_width: TotalFloat(params.width),
            last_pos: pos.into(),
        }
    }
}

pub fn immediate_draw_box_edges(canvas: &mut LineCanvas, aabb: &Aabb) {
    let [nnn, nnp, npn, npp, pnn, pnp, ppn, ppp] = aabb_corners(aabb);

    // bottom
    canvas.goto(nnn).line(nnp).line(pnp).line(pnn).line(nnn);
    // top
    canvas.goto(npn).line(npp).line(ppp).line(ppn).line(npn);

    // connecting lines
    canvas.goto(nnn).line(npn);
    canvas.goto(nnp).line(npp);
    canvas.goto(pnp).line(ppp);
    canvas.goto(pnn).line(ppn);
}

fn debug_lines_camera(canvas: &mut LineCanvas, transform: &Transform, camera: &Camera) {
    const K: f32 = 1.5;

    let half_fovy = camera.projection.fovy() / 2.0;
    let inv_aspect = 1.0 / camera.projection.aspect();

    let distance = K * f32::cos(half_fovy);
    let hypot = 2.0 * K * f32::tan(half_fovy);

    let aspect_angle = f32::atan(inv_aspect);
    let base = hypot * f32::cos(aspect_angle) / 2.0;
    let height = hypot * f32::sin(aspect_angle) / 2.0;

    let to_world = transform.to_matrix();

    let origin = transform.pos();
    let nn = (to_world * vector![-base, -height, -distance, 1.0]).xyz();
    let np = (to_world * vector![-base, height, -distance, 1.0]).xyz();
    let pn = (to_world * vector![base, -height, -distance, 1.0]).xyz();
    let pp = (to_world * vector![base, height, -distance, 1.0]).xyz();

    let tri_left = (to_world * vector![-0.2, height + 0.2, -distance, 1.0]).xyz();
    let tri_right = (to_world * vector![0.2, height + 0.2, -distance, 1.0]).xyz();
    let tri_top = (to_world * vector![0.0, height + 0.4, -distance, 1.0]).xyz();

    canvas.width(1.0);

    canvas.goto(origin);
    canvas.line(nn).line(np).line(origin);
    canvas.line(pp).line(pn).line(origin);
    canvas.goto(nn).line(pn);
    canvas.goto(pp).line(np);

    canvas
        .color([1.0, 0.8, 0.3, 1.0])
        .goto(tri_top)
        .line(tri_left)
        .line(tri_right)
        .line(tri_top);
}

fn aabb_corners(aabb: &Aabb) -> [Vector3<f32>; 8] {
    [
        vector![aabb.min.x, aabb.min.y, aabb.min.z],
        vector![aabb.min.x, aabb.min.y, aabb.max.z],
        vector![aabb.min.x, aabb.max.y, aabb.min.z],
        vector![aabb.min.x, aabb.max.y, aabb.max.z],
        vector![aabb.max.x, aabb.min.y, aabb.min.z],
        vector![aabb.max.x, aabb.min.y, aabb.max.z],
        vector![aabb.max.x, aabb.max.y, aabb.min.z],
        vector![aabb.max.x, aabb.max.y, aabb.max.z],
    ]
}

fn debug_lines_debug_box(canvas: &mut LineCanvas, debug_box: &DebugBox) {
    canvas.color(debug_box.rgba);
    immediate_draw_box_edges(canvas, &debug_box.bounds);
}

fn begin_render(mut ctx: RenderParams) -> anyhow::Result<()> {
    ctx.targets.reset()?;
    ctx.targets.frame = Some(ctx.display().draw());
    Ok(())
}

fn end_render(mut ctx: RenderParams) -> anyhow::Result<()> {
    let frame = ctx.targets.frame.take().unwrap();
    let result_buf = ctx.targets.get("final")?.framebuffer(ctx.display())?;
    result_buf.fill(&frame, MagnifySamplerFilter::Linear);
    frame.finish()?;
    Ok(())
}

fn add_global_debug_lines(mut lines: ResMut<ImmediateLines>, mut debug: NonSendMut<DebugLines>) {
    let mut canvas = lines.start_default();
    let debug = &mut *debug;

    for debug_box in debug.debug_box_channel.rx.try_iter() {
        debug_lines_debug_box(&mut canvas, &debug_box);
    }

    for (duration, debug_box) in debug.transient_debug_box_channel.rx.try_iter() {
        debug.transient_debug_boxes.insert(
            debug.next_transient_id,
            (Instant::now(), duration, debug_box),
        );
        debug.next_transient_id += 1;
        debug_lines_debug_box(&mut canvas, &debug_box);
    }

    for (&i, (start, duration, debug_box)) in debug.transient_debug_boxes.iter_mut() {
        let elapsed = start.elapsed();
        if elapsed > *duration {
            debug.dead_transient_debug_boxes.insert(i);
        } else {
            let percent_completed = elapsed.as_secs_f32() / duration.as_secs_f32();
            let mut rgba = debug_box.rgba;
            rgba[3] *= 1.0 - percent_completed;
            debug_lines_debug_box(&mut canvas, &DebugBox { rgba, ..*debug_box });
        }
    }

    for i in debug.dead_transient_debug_boxes.drain() {
        debug.transient_debug_boxes.remove(&i);
    }
}

fn render_lines(
    mut ctx: RenderParams,
    camera: CurrentCamera,
    mut lines: ResMut<ImmediateLines>,
) -> anyhow::Result<()> {
    // i wonder if it would be faster to use the GL_LINES and issue one draw call
    // for each line weight, or to use GL_TRIANGLES and build the line geometry
    // manually, drawing only once. for now, using GL_LINES is easier, so ill stick
    // with that.

    let vertices = VertexBuffer::immutable(ctx.display(), &lines.vertices)?;
    lines.vertices.clear();

    for (&TotalFloat(width), buffer) in lines.lines.iter_mut() {
        let indices = IndexBuffer::immutable(ctx.display(), PrimitiveType::LinesList, &buffer)?;
        let view = camera.view();
        let proj = camera.projection(ctx.display.get_framebuffer_dimensions());

        let mut target = ctx.targets.get("world")?.framebuffer(ctx.display())?;
        let program = ctx.shaders.get("debug")?;

        target.draw(
            &vertices,
            &indices,
            &program,
            &uniform! {
                view: array4x4(&view),
                projection: array4x4(&proj.to_homogeneous()),
            },
            &DrawParameters {
                line_width: Some(width),
                blend: Blend::alpha_blending(),
                depth: glium::Depth {
                    test: glium::DepthTest::IfLess,
                    write: false,
                    ..Default::default()
                },
                ..Default::default()
            },
        )?;

        buffer.clear();
    }

    Ok(())
}

fn render_post(
    mut ctx: RenderParams,
    camera: CurrentCamera,
    misc: NonSend<RendererMisc>,
    mut time: ShaderTime,
) -> anyhow::Result<()> {
    let (elapsed_seconds, elapsed_subseconds) = time.get();
    let program = ctx.shaders.get("post")?;

    let world_buffer = ctx.targets.get("world")?;
    let color = world_buffer
        .color()
        .unwrap()
        .uniform()?
        .magnify_filter(MagnifySamplerFilter::Nearest)
        .anisotropy(4);
    let depth = world_buffer
        .depth()
        .unwrap()
        .uniform()?
        .magnify_filter(MagnifySamplerFilter::Nearest)
        .anisotropy(4);

    let mut final_buffer = ctx.targets.get("final")?.framebuffer(ctx.display())?;
    final_buffer.clear_color(0.0, 0.0, 0.0, 0.0);

    let dimensions = ctx.display().get_framebuffer_dimensions();
    let proj = camera.projection(dimensions);

    final_buffer.draw(
        &misc.fullscreen_quad,
        glium::index::NoIndices(PrimitiveType::TrianglesList),
        &program,
        &uniform! {
            colorBuffer: color,
            depthBuffer: depth,

            elapsedSeconds: elapsed_seconds,
            elapsedSubseconds: elapsed_subseconds,
            screenDimensions: [dimensions.0, dimensions.1],

            cameraPosWorld: array3(&camera.pos()),
            projectionMatrix: array4x4(&proj.to_homogeneous()),
            viewMatrix: array4x4(&camera.view()),
        },
        &Default::default(),
    )?;

    let (width, height) = ctx.display().get_framebuffer_dimensions();
    let program = ctx.shaders.get("crosshair")?;
    final_buffer.draw(
        &misc.fullscreen_quad,
        glium::index::NoIndices(PrimitiveType::TrianglesList),
        &program,
        &uniform! {
            screen_width: width as f32,
            screen_height: height as f32,
            crosshair_texture: misc.crosshair_texture.sampled().magnify_filter(MagnifySamplerFilter::Nearest),
        },
        &glium::DrawParameters {
            blend: Blend::alpha_blending(),
            ..Default::default()
        },
    )?;

    Ok(())
}

fn render_sky(
    mut ctx: RenderParams,
    camera: CurrentCamera,
    misc: NonSend<RendererMisc>,
    mut time: ShaderTime,
) -> anyhow::Result<()> {
    let (elapsed_seconds, elapsed_subseconds) = time.get();
    let program = ctx.shaders.get("sky")?;
    let mut target = ctx.targets.get("world")?.framebuffer(ctx.display())?;

    let dimensions = ctx.display().get_framebuffer_dimensions();
    let proj = camera.projection(dimensions);
    target.draw(
        &misc.fullscreen_quad,
        glium::index::NoIndices(PrimitiveType::TrianglesList),
        &program,
        &uniform! {
            elapsedSeconds: elapsed_seconds,
            elapsedSubseconds: elapsed_subseconds,
            screenDimensions: [dimensions.0, dimensions.1],
            cameraPosWorld: array3(&camera.pos()),
            projectionMatrix: array4x4(&proj.to_homogeneous()),
            viewMatrix: array4x4(&camera.view()),
        },
        &Default::default(),
    )?;

    Ok(())
}

#[derive(SystemParam)]
pub struct ShaderTime<'a> {
    time: Res<'a, Time>,
    elapsed_seconds: Local<'a, u32>,
    elapsed_subseconds: Local<'a, f32>,
}

impl<'a> ShaderTime<'a> {
    pub fn get(&mut self) -> (u32, f32) {
        *self.elapsed_subseconds += self.time.delta_seconds();
        while *self.elapsed_subseconds >= 1.0 {
            *self.elapsed_seconds += 1;
            *self.elapsed_subseconds -= 1.0;
        }

        (*self.elapsed_seconds, *self.elapsed_subseconds)
    }
}

fn render_terrain(
    mut ctx: RenderParams,
    camera: CurrentCamera,
    mesh_query: Query<(&Transform, &RenderMeshComponent<TerrainMesh>)>,
    mut terrain_meshes: NonSendMut<LocalMeshContext<TerrainMesh>>,
    misc: NonSend<RendererMisc>,
    mut time: ShaderTime,
) -> anyhow::Result<()> {
    terrain_meshes.update(ctx.display())?;

    let (elapsed_seconds, elapsed_subseconds) = time.get();
    let mut target = ctx.targets.get("world")?.framebuffer(ctx.display())?;
    let program = ctx.shaders.get("terrain")?;

    let view = camera.view();
    let proj = camera.projection(ctx.display.get_framebuffer_dimensions());
    let viewproj = proj.as_matrix() * view;

    for (transform, RenderMeshComponent(handle)) in mesh_query.iter() {
        let buffers = terrain_meshes
            .meshes
            .get(&handle.0.id)
            .expect("RenderMeshComponent existed for entity that was not in terrain_meshes");

        let model = transform.to_matrix();
        let mvp = viewproj * model;

        if !should_draw_aabb(&mvp, &buffers.aabb) {
            continue;
        }

        target.draw(
            &buffers.vertices,
            &buffers.indices,
            &program,
            &uniform! {
            model: array4x4(&model),
            view: array4x4(&view),
            projection: array4x4(&proj.to_homogeneous()),
            albedo_maps: misc.block_textures.sampled()
                .wrap_function(glium::uniforms::SamplerWrapFunction::Repeat)
                .magnify_filter(MagnifySamplerFilter::Nearest),
                elapsedSeconds: elapsed_seconds,
                elapsedSubseconds: elapsed_subseconds,
            },
            &glium::DrawParameters {
                depth: glium::Depth {
                    test: glium::DepthTest::IfLess,
                    write: true,
                    ..Default::default()
                },
                backface_culling: glium::BackfaceCullingMode::CullCounterClockwise,
                // polygon_mode: glium::PolygonMode::Line,
                ..Default::default()
            },
        )?;
    }

    Ok(())
}
