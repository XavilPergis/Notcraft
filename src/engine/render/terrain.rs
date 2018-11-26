use engine::{camera::Camera, prelude::*, render::mesh::Mesh};
use gl_api::{
    context::Context,
    shader::{load_shader, program::Program},
    texture_array::TextureArray2d,
};
use glutin::GlWindow;

vertex! {
    vertex BlockVertex {
        pos: Point3<f32>,
        normal: Vector3<f32>,
        uv: Vector2<f32>,
        tex_id: i32,
        ao: f32,
    }
}

pub type ChunkMesh = Mesh<BlockVertex, u32>;

pub struct TerrainRenderer {
    ctx: Context,
    program: Program,
    textures: TextureArray2d,
}

impl TerrainRenderer {
    pub fn new(ctx: &mut Context, names: Vec<String>) -> Self {
        let textures = TextureArray2d::new(ctx, 16, 16, names.len());
        let images: Result<Vec<_>, _> = names
            .into_iter()
            .map(|name| {
                info!("trying to open resources/textures/{}", &name);
                image::open(format!("resources/textures/{}", name))
            })
            .collect();
        // FIXME: omg propogate this error plz
        textures.upload_textures(ctx, images.unwrap().into_iter().map(|img| img.to_rgb()));
        // let textures = Texture2D::new();
        // textures
        //     .source_from_image("resources/textures.png")
        //     .unwrap();
        // textures.min_filter(MinimizationFilter::Nearest);
        // textures.mag_filter(MagnificationFilter::Nearest);
        // textures.texture_wrap_behavior(TextureAxis::R, WrapMode::Repeat);
        // textures.texture_wrap_behavior(TextureAxis::S, WrapMode::Repeat);
        // textures.set_texture_bank(0);

        let mut program = load_shader(ctx, "resources/terrain.vs", "resources/terrain.fs");
        program.set_uniform(ctx, "time", &0.0f32);
        program.set_uniform(ctx, "ambient_light", &Vector3::<f32>::new(0.8, 0.8, 0.8));
        program.set_uniform(ctx, "camera_position", &Vector3::new(0.0f32, 10.0, 0.0));
        program.set_uniform(ctx, "texture_map", &textures);

        TerrainRenderer {
            ctx: ctx.clone(),
            program,
            textures,
        }
    }
}

impl<'a> System<'a> for TerrainRenderer {
    type SystemData = (
        WriteStorage<'a, ChunkMesh>,
        ReadStorage<'a, comp::Transform>,
        ReadExpect<'a, Camera>,
    );

    fn run(&mut self, (mut meshes, transforms, camera): Self::SystemData) {
        use gl_api::buffer::UsageType;

        for mesh in (&mut meshes).join() {
            if mesh.needs_new_gpu_mesh() {
                mesh.upload(&self.ctx, UsageType::StaticDraw).unwrap();
            }
        }

        let projection = camera.projection_matrix().cast::<f32>().unwrap();
        self.program
            .set_uniform(&mut self.ctx, "projection_matrix", &projection);
        self.program.set_uniform(
            &mut self.ctx,
            "camera_position",
            &camera.position.cast::<f32>().unwrap(),
        );
        for (mesh, tfm) in (&meshes, &transforms).join() {
            let tfm: Matrix4<f32> = tfm.model_matrix().cast::<f32>().unwrap();
            let view_matrix: Matrix4<f32> = camera.view_matrix().cast().unwrap();
            self.program
                .set_uniform(&mut self.ctx, "view_matrix", &view_matrix);
            self.program
                .set_uniform(&mut self.ctx, "model_matrix", &tfm);
            if let Some(mesh) = mesh.gpu_mesh.as_ref() {
                mesh.draw_with(&mut self.ctx, &self.program);
            }
        }
    }
}
