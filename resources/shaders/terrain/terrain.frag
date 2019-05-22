#version 330 core

uniform sampler2DArray albedo_maps;
uniform sampler2DArray normal_maps;
uniform sampler2DArray extra_maps;

in vs_out {
    vec3 v_pos;
    vec3 v_pos_world;
    vec2 v_uv;
    vec3 v_norm;
    vec3 v_tang;
    float v_ao;
} fs_in;
flat in int v_id;

out vec3 gposition;
out vec3 gcolor;
out vec3 gnormal;
out vec3 gemissive;
out vec2 gextra;

void main()
{
    // world-space position g-buffer; multiply by model matrix to transform to world space
    // position = model_matrix * vec4(v_pos_model, 1.0);

    vec3 tex_pos = vec3(fs_in.v_uv, v_id);

    vec3 albedo = texture(albedo_maps, tex_pos).rgb;
    vec3 extra = texture(extra_maps, tex_pos).rgb;
    vec3 normal_tang = 2.0 * texture(normal_maps, tex_pos).xyz - 1.0;
    vec3 bitangent_model = normalize(cross(fs_in.v_norm, fs_in.v_tang));

    mat3 tang_to_model = mat3(
        fs_in.v_tang,
        bitangent_model,
        fs_in.v_norm
    );

    // // world-space normal g-buffer
    gposition = fs_in.v_pos_world;
    gnormal = tang_to_model * normal_tang;
    gcolor = albedo;
    gextra = vec2(1.0 - extra.r, extra.g);

    gemissive = albedo * extra.b;
    // gcolor = vec3(1.0);
}
