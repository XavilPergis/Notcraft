#version 330 core

#define MIN_AO 0.5
#define AO_CURVE 0.8

uniform sampler2DArray albedo_maps;
uniform sampler2DArray normal_maps;
uniform sampler2DArray height_maps;
uniform sampler2DArray roughness_maps;
uniform sampler2DArray ao_maps;
uniform sampler2DArray metallic_maps;

uniform mat4 model_matrix;
uniform mat4 projection_matrix;
uniform mat4 view_matrix;

in vec3 v_pos_model;
in vec3 v_normal_model;
in vec3 v_tangent_model;
in vec3 v_face_scalar;
in vec2 v_uv;
flat in int v_tex_id;
in float v_ao;

out vec4 positions;
out vec4 normals;
out vec4 colors;
out vec4 extra;

vec2 uv_wrap(vec2 uv) {
    return vec2(mod(uv.x, 1.0), mod(uv.y, 1.0));
}

void main()
{
    // world-space position g-buffer; multiply by model matrix to transform to world space
    positions = model_matrix * vec4(v_pos_model, 1.0);

    vec3 tex_pos = vec3(uv_wrap(v_uv), float(v_tex_id));

    vec4 color = texture(albedo_maps, tex_pos);
    vec3 normal_tang = 2.0 * texture(normal_maps, tex_pos).xyz - vec3(1.0, 1.0, 1.0);
    vec3 bitangent_model = normalize(cross(v_normal_model, v_tangent_model));

    mat3 tang_to_model = mat3(
        v_tangent_model,
        bitangent_model,
        v_normal_model
    );

    extra = vec4(texture(roughness_maps, tex_pos).r, texture(metallic_maps, tex_pos).r, 0.0, 0.0);
    // extra = vec4(0.0, 1.0, 0.0, 1.0);
    normals = vec4(tang_to_model * normal_tang, 0.0);
    colors = vec4(v_face_scalar * v_ao * color.rgb, color.a);
}
