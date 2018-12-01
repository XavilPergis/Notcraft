extern crate notcraft_graphics;

use glutin::{dpi::*, GlContext};
use notcraft_graphics::{
    self as graphics, Buffer, BufferBuilder, Context, PrimitiveType, ProgramStages, UsageType,
};

fn main() {
    let mut events_loop = glutin::EventsLoop::new();
    let window = glutin::WindowBuilder::new()
        .with_title("Hello, world!")
        .with_dimensions(LogicalSize::new(1024.0, 768.0));
    let glutin_context = glutin::ContextBuilder::new().with_vsync(true);
    let window = glutin::GlWindow::new(window, glutin_context, &events_loop).unwrap();

    unsafe {
        window.make_current().unwrap();
    }

    let mut ctx = Context::load(|symbol| window.get_proc_address(symbol));

    let positions = [[-0.5, -0.5], [0.0, 0.5], [0.5, -0.5f32]];
    let colors = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0f32]];

    let foo = [
        ([-0.5, -0.5], ([1.0, 0.0, 0.0], [0.5, 0.5])),
        ([0.0, 0.5], ([0.0, 1.0, 0.0], [0.5, 0.5])),
        ([0.5, -0.5f32], ([0.0, 0.0, 1.0], [0.5, 0.5])),
    ];
    let bar = [0.3, 0.6, 0.9];

    type Vertex = graphics::cons_ty![([f32; 2], ([f32; 3], [f32; 2])), f32];

    let pos_buffer = BufferBuilder::new()
        .with_usage(UsageType::StaticDraw)
        .with_data(&foo[..])
        .build(&ctx)
        .unwrap();

    let col_buffer = BufferBuilder::new()
        .with_usage(UsageType::StaticDraw)
        .with_data(&bar[..])
        .build(&ctx)
        .unwrap();

    let stages = ProgramStages::new(
        &ctx,
        r#"#version 330 core

layout (location = 0) in vec2 pos;
layout (location = 1) in vec3 color;
layout (location = 2) in vec2 foo;
layout (location = 3) in float bar;

out vec3 v_color;
out float v_bar;

void main() {
    gl_Position = vec4(pos + foo, 0.0, 1.0);
    v_color = color;
    v_bar = bar;
}
"#,
        r#"#version 330 core

out vec4 out_color;
in float v_bar;
in vec3 v_color;

void main() {
    out_color = vec4(v_bar * v_color, 1.0);
}
"#,
    )
    .build()
    .unwrap();

    let program = graphics::create_program::<Vertex>(&ctx, stages).unwrap();

    let mut running = true;
    while running {
        events_loop.poll_events(|event| match event {
            glutin::Event::WindowEvent { event, .. } => match event {
                glutin::WindowEvent::CloseRequested => running = false,
                glutin::WindowEvent::Resized(logical_size) => {
                    let dpi_factor = window.get_hidpi_factor();
                    window.resize(logical_size.to_physical(dpi_factor));
                }
                _ => (),
            },
            _ => (),
        });

        ctx.clear_color(0.5, 0.5, 0.5, 1.0);

        ctx.draw_arrays(
            PrimitiveType::Triangles,
            &program,
            graphics::cons![&pos_buffer, &col_buffer],
        );

        window.swap_buffers().unwrap();

        // break;
    }
}
