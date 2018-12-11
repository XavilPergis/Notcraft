use crate::engine::{
    camera::Camera,
    prelude::*,
    render::{DeferredRenderPass, DeferredRenderPassContext, GraphicsData},
};
use glium::{
    backend::Facade,
    texture::{RawImage2d, Texture2dArray},
    uniforms::{MagnifySamplerFilter, MinifySamplerFilter},
    *,
};
use std::{cell::RefCell, rc::Rc};

pub struct TerrainRenderer {
    ctx: Display,
    graphics: Rc<RefCell<GraphicsData>>,
    terrain_program: Program,
    water_program: Program,
}

impl TerrainRenderer {
    pub fn new(ctx: &Display, graphics: &Rc<RefCell<GraphicsData>>) -> std::io::Result<Self> {
        let terrain_program = Program::from_source(
            ctx,
            &util::read_file("resources/shaders/terrain.vs")?,
            &util::read_file("resources/shaders/terrain.fs")?,
            None,
        )
        .unwrap();

        let water_program = Program::from_source(
            ctx,
            &util::read_file("resources/shaders/water.vs")?,
            &util::read_file("resources/shaders/water.fs")?,
            None,
        )
        .unwrap();

        Ok(TerrainRenderer {
            ctx: ctx.clone(),
            graphics: graphics.clone(),
            terrain_program,
            water_program,
        })
    }
}

impl DeferredRenderPass for TerrainRenderer {
    fn draw<F: Facade>(
        &mut self,
        ctx: &mut DeferredRenderPassContext<'_, '_, '_, F>,
    ) -> Result<(), glium::DrawError> {
        for (pos, mesh) in ctx.data.iter_terrain() {
            if let Some(gpu) = mesh.gpu.as_ref() {
                let world_pos = pos.base().base();
                let model: [[f32; 4]; 4] =
                    Matrix4::from_translation(util::to_vector(world_pos.0)).into();
                let ambient_light = [1.0, 1.0, 1.0f32];

                let albedo_maps = ctx
                    .data
                    .albedo_maps
                    .sampled()
                    .magnify_filter(MagnifySamplerFilter::Linear)
                    .minify_filter(MinifySamplerFilter::LinearMipmapNearest);
                let normal_maps = ctx
                    .data
                    .normal_maps
                    .sampled()
                    .magnify_filter(MagnifySamplerFilter::Linear)
                    .minify_filter(MinifySamplerFilter::LinearMipmapNearest);
                let height_maps = ctx
                    .data
                    .height_maps
                    .sampled()
                    .magnify_filter(MagnifySamplerFilter::Linear)
                    .minify_filter(MinifySamplerFilter::LinearMipmapNearest);
                let roughness_maps = ctx
                    .data
                    .roughness_maps
                    .sampled()
                    .magnify_filter(MagnifySamplerFilter::Linear)
                    .minify_filter(MinifySamplerFilter::LinearMipmapNearest);
                let ao_maps = ctx
                    .data
                    .ao_maps
                    .sampled()
                    .magnify_filter(MagnifySamplerFilter::Linear)
                    .minify_filter(MinifySamplerFilter::LinearMipmapNearest);
                let metallic_maps = ctx
                    .data
                    .metallic_maps
                    .sampled()
                    .magnify_filter(MagnifySamplerFilter::Linear)
                    .minify_filter(MinifySamplerFilter::LinearMipmapNearest);

                let uniforms = glium::uniform! {
                    time: 0.0,
                    model_matrix: model,
                    view_matrix: ctx.view_matrix(),
                    projection_matrix: ctx.projection_matrix(),
                    camera_position: ctx.eye_pos(),
                    ambient_light: ambient_light,

                    albedo_maps: albedo_maps,
                    normal_maps: normal_maps,
                    height_maps: height_maps,
                    roughness_maps: roughness_maps,
                    ao_maps: ao_maps,
                    metallic_maps: metallic_maps,
                };

                ctx.target.draw(
                    &gpu.vertices,
                    &gpu.indices,
                    &self.terrain_program,
                    &uniforms,
                    &ctx.default_draw_params(),
                )?;
            }
        }
        Ok(())
    }
}

// impl<'a> System<'a> for TerrainRenderer {
//     type SystemData = (
//         WriteStorage<'a, TerrainMeshes>,
//         ReadStorage<'a, comp::Transform>,
//         ReadExpect<'a, Camera>,
//     );

//     fn run(&mut self, (mut meshes, transforms, camera): Self::SystemData) {
//         for mesh in (&mut meshes).join() {
//             if mesh.terrain.needs_new_gpu_mesh() {
//                 mesh.terrain
//                     .upload(&self.ctx, UsageType::StaticDraw)
//                     .unwrap();
//             }

//             if mesh.liquid.needs_new_gpu_mesh() {
//                 mesh.liquid
//                     .upload(&self.ctx, UsageType::StaticDraw)
//                     .unwrap();
//             }
//         }

//         let projection = camera.projection_matrix().cast::<f32>().unwrap();
//         self.terrain_program
//             .set_uniform(&mut self.ctx, "projection_matrix", &projection);
//         self.terrain_program.set_uniform(
//             &mut self.ctx,
//             "camera_position",
//             &camera.position.cast::<f32>().unwrap(),
//         );
//         self.water_program
//             .set_uniform(&mut self.ctx, "projection_matrix", &projection);
//         self.water_program.set_uniform(
//             &mut self.ctx,
//             "camera_position",
//             &camera.position.cast::<f32>().unwrap(),
//         );

//         // Draw terrain
//         for (mesh, tfm) in (&meshes, &transforms).join() {
//             let tfm: Matrix4<f32> =
// tfm.model_matrix().cast::<f32>().unwrap();             let view_matrix:
// Matrix4<f32> = camera.view_matrix().cast().unwrap();

//             if let Some(mesh) = mesh.terrain.gpu_mesh.as_ref() {
//                 self.terrain_program
//                     .set_uniform(&mut self.ctx, "view_matrix", &view_matrix);
//                 self.terrain_program
//                     .set_uniform(&mut self.ctx, "model_matrix", &tfm);
//                 mesh.draw_with(&mut self.ctx, &self.terrain_program);
//             }
//         }

//         // Draw water
//         for (mesh, tfm) in (&meshes, &transforms).join() {
//             let tfm: Matrix4<f32> =
// tfm.model_matrix().cast::<f32>().unwrap();             let view_matrix:
// Matrix4<f32> = camera.view_matrix().cast().unwrap();

//             if let Some(mesh) = mesh.liquid.gpu_mesh.as_ref() {
//                 self.water_program
//                     .set_uniform(&mut self.ctx, "view_matrix", &view_matrix);
//                 self.water_program
//                     .set_uniform(&mut self.ctx, "model_matrix", &tfm);
//                 mesh.draw_with(&mut self.ctx, &self.water_program);
//             }
//         }
//     }
// }
