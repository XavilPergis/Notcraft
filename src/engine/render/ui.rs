use engine::{prelude::*, render::verts};
use gl_api::{
    buffer::Buffer,
    context::Context,
    shader::{load_shader, program::Program},
    texture::Texture2d,
    PrimitiveType, UsageType,
};
use glutin::GlWindow;

fn gen_quad(ctx: &Context) -> Buffer<verts::PosUv> {
    let mut buf = Buffer::new(ctx);
    buf.upload(ctx, verts::UV_QUAD_CW, UsageType::StaticDraw)
        .unwrap();
    buf
}

pub struct DrawCrosshair {
    ctx: Context,
    texture: Texture2d,
    program: Program,
    buffer: Buffer<verts::PosUv>,
}

impl DrawCrosshair {
    pub fn new(ctx: &Context) -> Self {
        let texture = Texture2d::from_image(
            ctx,
            &::image::open("resources/crosshair.png").unwrap().to_rgba(),
        );

        let mut program = load_shader(
            ctx,
            "resources/simple_texture.vs",
            "resources/simple_texture.fs",
        );
        program.set_uniform(ctx, "tex", &texture);

        DrawCrosshair {
            ctx: ctx.clone(),
            texture,
            program,
            buffer: gen_quad(ctx),
        }
    }
}

impl<'a> System<'a> for DrawCrosshair {
    type SystemData = (ReadExpect<'a, GlWindow>);

    fn run(&mut self, window: Self::SystemData) {
        let size = window.get_inner_size().unwrap();
        let size: (f64, f64) = size.to_physical(window.get_hidpi_factor()).into();
        let size = (size.0 as f32, size.1 as f32);

        self.program.set_uniform(&self.ctx, "resolution", &size);

        self.ctx
            .draw_arrays(PrimitiveType::Triangles, &self.program, &self.buffer);
    }
}
