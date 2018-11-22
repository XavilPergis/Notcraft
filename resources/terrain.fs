#version 330 core

#define MIN_AO 0.5
#define AO_CURVE 0.8

uniform vec3 u_CameraPosition;
uniform vec3 u_LightAmbient;
uniform sampler2DArray u_TextureMap;

in vec3 v_Pos;
in vec3 v_Normal;
in vec3 v_FaceScalar;
in vec2 v_Uv;
flat in int v_TexID;
in float v_Ao;

out vec4 color;

void main()
{
    float density = 0.007;
    float gradient = 5.0;
    float fog = exp(-pow(length(u_CameraPosition - v_Pos) * density, gradient));
    vec4 tex_color = texture(u_TextureMap, vec3(v_Uv, float(v_TexID)));
    // return ((n-start1)/(stop1-start1))*(stop2-start2)+start2;
    float ao = pow(v_Ao, 1.0 / AO_CURVE) * (1.0 - MIN_AO) + MIN_AO;
    vec4 col = vec4(v_FaceScalar, 1.0) * ao * tex_color * vec4(u_LightAmbient, 1.0);

    color = mix(vec4(0.729411765, 0.907843137, 0.981568627, 1.0), col, fog);
}
