use engine::{prelude::*, render::mesh::Mesh};
use gl_api::{
    context::Context,
    shader::{load_shader, program::LinkedProgram},
    texture::*,
};
use glutin::GlWindow;

vertex! {
    vertex BlockVertex {
        pos: Point3<f32>,
        normal: Vector3<f32>,
        face: i32,
        tile_offset: Vector2<f32>,
        uv: Vector2<f32>,
        ao: f32,
    }
}

pub type ChunkMesh = Mesh<BlockVertex, u32>;

pub struct TerrainRenderer {
    ctx: Context,
    program: LinkedProgram,
    textures: Texture2D,
}

impl TerrainRenderer {
    pub fn new(ctx: &Context) -> Self {
        let textures = Texture2D::new();
        textures
            .source_from_image("resources/textures.png")
            .unwrap();
        textures.min_filter(MinimizationFilter::Nearest);
        textures.mag_filter(MagnificationFilter::Nearest);
        textures.texture_wrap_behavior(TextureAxis::R, WrapMode::Repeat);
        textures.texture_wrap_behavior(TextureAxis::S, WrapMode::Repeat);
        textures.set_texture_bank(0);

        let mut program = load_shader("resources/terrain.vs", "resources/terrain.fs");
        program.set_uniform("u_Time", &0.0f32);
        program.set_uniform("u_LightAmbient", &Vector3::<f32>::new(0.8, 0.8, 0.8));
        program.set_uniform("u_CameraPosition", &Vector3::new(0.0f32, 10.0, 0.0));
        program.set_uniform("u_TextureMap", &textures);

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
        ReadStorage<'a, comp::Player>,
        ReadStorage<'a, comp::ClientControlled>,
        ReadExpect<'a, GlWindow>,
        ReadExpect<'a, res::ViewFrustum>,
    );

    fn run(
        &mut self,
        (
            mut meshes,
            transforms,
            player_marker,
            client_controlled_marker,
            window,
            frustum,
        ): Self::SystemData,
    ) {
        let player_transform = (&player_marker, &client_controlled_marker, &transforms)
            .join()
            .map(|(_, _, tfm)| tfm)
            .next();

        use gl_api::buffer::UsageType;

        for mesh in (&mut meshes).join() {
            if mesh.needs_new_gpu_mesh() {
                mesh.upload(&self.ctx, UsageType::StaticDraw);
            }
        }

        if let Some(player_transform) = player_transform {
            let aspect_ratio = ::util::aspect_ratio(&window).unwrap() as f32;
            let projection = ::cgmath::perspective(
                Deg(frustum.fov.0 as f32),
                aspect_ratio,
                frustum.near_plane as f32,
                frustum.far_plane as f32,
            );
            self.program.set_uniform("u_Projection", &projection);
            self.program.set_uniform(
                "u_CameraPosition",
                &player_transform.position.cast::<f32>().unwrap(),
            );
            for (mesh, tfm) in (&meshes, &transforms).join() {
                let tfm: Matrix4<f32> = tfm.as_matrix().cast::<f32>().unwrap();
                let view_matrix: Matrix4<f32> = player_transform.as_matrix().cast::<f32>().unwrap();
                self.program.set_uniform("u_View", &view_matrix);
                self.program.set_uniform("u_Transform", &tfm);
                if let Some(mesh) = mesh.gpu_mesh.as_ref() {
                    mesh.draw_with(&self.ctx, &self.program);
                }
            }
        }
    }
}