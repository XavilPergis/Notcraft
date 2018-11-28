trait GlObject {
    unsafe fn create(ctx: &Context) -> Self;
}

// ctx.create::<Framebuffer>()

pub struct RawFramebuffer {
    id: u32,
}

impl RawFramebuffer {}

#[derive(Debug)]
pub struct Framebuffer {
    crate raw: RawFramebuffer,
}

impl Framebuffer {}
