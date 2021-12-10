#version 330 core

in vec3 pos;
in vec2 uv;
in vec3 normal;
in vec3 tangent;
in float ao;
in int id;

uniform mat4 model;
uniform mat4 view;
uniform mat4 projection;

out vec3 v_color_filter;
out vec2 v_texture_uv;
flat out int v_id;

void main()
{
    gl_Position = projection * view * model * vec4(pos, 1.0);

    float ao_strength = (1.0 - ao) * 0.9;

    v_id = id;
    v_color_filter = vec3(1.0 - ao_strength);
    v_texture_uv = uv;
}
