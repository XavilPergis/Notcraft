#version 330 core

#define MAX_LIGHTS 4

uniform vec3 u_CameraPosition;
uniform vec3 u_LightAmbient;
uniform sampler2D u_TextureMap;

in vec3 v_Pos;
in vec3 v_Normal;
in float v_FaceScalar;
in vec2 v_Uv;
in vec2 v_Tile;

out vec4 color;

void main()
{
    vec2 tex_coords = v_Tile / 4.0 + (fract(v_Uv) / 4.0);

    float density = 0.007;
    float gradient = 2.3;
    float fog = exp(-pow(length(u_CameraPosition - v_Pos) * density, gradient));
    vec4 tex_color = texture(u_TextureMap, tex_coords);
    vec4 col = v_FaceScalar * tex_color * vec4(u_LightAmbient, 1.0);

    color = mix(vec4(0.729411765, 0.907843137, 0.981568627, 1.0), col, fog);
}
