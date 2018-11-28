#version 330 core

layout (location = 0) in vec3 pos;
layout (location = 1) in vec3 normal;
layout (location = 2) in vec2 uv;
layout (location = 3) in int tex_id;

uniform float time;

uniform mat4 model_matrix;
uniform mat4 projection_matrix;
uniform mat4 view_matrix;

out vec3 v_normal;
out vec3 v_pos;
out vec3 v_face_scalar;
out vec2 v_uv;
flat out int v_tex_id;

void main()
{
    gl_Position = projection_matrix * view_matrix * model_matrix * vec4(pos, 1.0);
    v_pos = vec3(model_matrix * vec4(pos, 1.0));
    v_normal = normal;
    v_uv = uv;
    v_tex_id = tex_id;

    if (normal.y == 1.0) v_face_scalar = vec3(1.0);
    if (normal.y == -1.0) v_face_scalar = vec3(0.5);
    if (abs(normal.x) == 1.0) v_face_scalar = vec3(0.70);
    if (abs(normal.z) == 1.0) v_face_scalar = vec3(0.80);
}
