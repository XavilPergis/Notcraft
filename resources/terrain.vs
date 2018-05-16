#version 330 core

layout (location = 0) in vec3 Pos;
layout (location = 1) in vec3 Normal;
layout (location = 2) in int Face;
layout (location = 3) in vec2 Tile;
layout (location = 4) in vec2 Uv;

uniform float u_Time;
uniform mat4 u_Transform;
uniform mat4 u_Projection;
uniform mat4 u_View;

out vec3 v_Normal;
out vec3 v_Pos;
out float v_FaceScalar;
out vec2 v_Uv;
out vec2 v_Tile;

void main()
{   
    gl_Position = u_Projection * u_View * u_Transform * vec4(Pos, 1.0);
    v_Pos = vec3(u_Transform * vec4(Pos, 1.0));
    v_Normal = Normal;
    v_Uv = Uv;
    v_Tile = Tile;

    switch (Face)
    {
        case 0: v_FaceScalar = 0.7; break;
        case 1: v_FaceScalar = 0.85; break;
        case 2: v_FaceScalar = 1.0; break;
        default: v_FaceScalar = 2.0;
    }
}
