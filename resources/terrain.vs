#version 330 core

layout (location = 0) in vec3 Pos;
layout (location = 1) in vec3 Normal;
layout (location = 2) in int Face;
layout (location = 3) in vec2 Tile;
layout (location = 4) in vec2 Uv;
layout (location = 5) in float Ao;

uniform float u_Time;
uniform mat4 u_Transform;
uniform mat4 u_Projection;
uniform mat4 u_View;

out vec3 v_Normal;
out vec3 v_Pos;
out vec3 v_FaceScalar;
out vec2 v_Uv;
out vec2 v_Tile;
out float v_Ao;

void main()
{
    gl_Position = u_Projection * u_View * u_Transform * vec4(Pos, 1.0);
    v_Pos = vec3(u_Transform * vec4(Pos, 1.0));
    v_Normal = Normal;
    v_Uv = Uv;
    v_Tile = Tile;
    v_Ao = Ao;

    if (Normal.y == 1.0) v_FaceScalar = vec3(1.0);
    if (Normal.y == -1.0) v_FaceScalar = vec3(0.5);
    if (abs(Normal.x) == 1.0 || abs(Normal.z) == 1.0) v_FaceScalar = vec3(0.75);
}
