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

out vs_out {
    // vec3 v_pos;
    // vec3 v_pos_world;
    vec2 v_uv;
    // vec3 v_norm;
    // vec3 v_tang;
    // float v_ao;
} output;
flat out int v_id;

void main()
{
    gl_Position = projection * view * model * vec4(pos, 1.0);

    v_id = id;
    // output.v_pos = pos;
    // output.v_pos_world = (model * vec4(pos, 1.0)).xyz;
    output.v_uv = uv;
    // output.v_norm = normal;
    // output.v_tang = tangent;
    // output.v_ao = ao;
}
