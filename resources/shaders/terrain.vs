#version 330 core

layout (location = 0) in vec3 pos;
layout (location = 1) in vec2 uv;
layout (location = 2) in vec3 normal;
layout (location = 3) in vec3 tangent;
layout (location = 4) in int tex_id;
layout (location = 5) in float ao;

uniform float time;

uniform mat4 model_matrix;
uniform mat4 projection_matrix;
uniform mat4 view_matrix;

out vec3 v_normal_model;
out vec3 v_tangent_model;
out vec3 v_pos_model;
out vec3 v_face_scalar;
out vec2 v_uv;
flat out int v_tex_id;
out float v_ao;

void main()
{
    gl_Position = projection_matrix * view_matrix * model_matrix * vec4(pos, 1.0);

    v_pos_model = pos;
    
    v_uv = uv;
    v_normal_model = normal;
    v_tangent_model = tangent;
    v_tex_id = tex_id;
    v_ao = ao;

    if (normal.y == 1.0) v_face_scalar = vec3(1.0);
    if (normal.y == -1.0) v_face_scalar = vec3(0.5);
    if (abs(normal.x) == 1.0) v_face_scalar = vec3(0.70);
    if (abs(normal.z) == 1.0) v_face_scalar = vec3(0.80);

    // v_face_scalar = vec3(0.2);
}
