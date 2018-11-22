#version 330 core

layout (location = 0) in vec3 Pos;
layout (location = 1) in vec3 Normal;
layout (location = 2) in vec2 Uv;
layout (location = 3) in int TexID;
layout (location = 4) in float Ao;

uniform float u_Time;
uniform mat4 u_Transform;
uniform mat4 u_Projection;
uniform mat4 u_View;

out vec3 v_Normal;
out vec3 v_Pos;
out vec3 v_FaceScalar;
out vec2 v_Uv;
flat out int v_TexID;
out float v_Ao;

void main()
{
    gl_Position = u_Projection * u_View * u_Transform * vec4(Pos, 1.0);
    v_Pos = vec3(u_Transform * vec4(Pos, 1.0));
    v_Normal = Normal;
    v_Uv = Uv;
    v_TexID = TexID;
    v_Ao = Ao;

    if (Normal.y == 1.0) v_FaceScalar = vec3(1.0);
    if (Normal.y == -1.0) v_FaceScalar = vec3(0.5);
    if (abs(Normal.x) == 1.0) v_FaceScalar = vec3(0.70);
    if (abs(Normal.z) == 1.0) v_FaceScalar = vec3(0.80);
}
