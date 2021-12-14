#version 330 core

uniform sampler2DArray albedo_maps;

in vec3 v_color_filter;
in vec2 v_texture_uv;
flat in int v_id;

out vec3 b_color;

void main()
{
    vec3 tex_pos = vec3(fract(v_texture_uv), v_id);
    vec3 albedo = v_color_filter * texture(albedo_maps, tex_pos).rgb;
    b_color = albedo;
}
