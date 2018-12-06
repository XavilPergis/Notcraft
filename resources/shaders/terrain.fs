#version 330 core

#define MIN_AO 0.5
#define AO_CURVE 0.8

uniform vec3 camera_position;
uniform vec3 ambient_light;
uniform sampler2DArray texture_map;

in vec3 v_pos;
in vec3 v_normal;
in vec3 v_face_scalar;
in vec2 v_uv;
flat in int v_tex_id;
in float v_ao;

out vec4 color;

vec2 uv_wrap(vec2 uv) {
    return vec2(mod(uv.x, 1.0), mod(uv.y, 1.0));
}

void main()
{
    float density = 0.003;
    float gradient = 5.0;
    float fog = exp(-pow(length(camera_position - v_pos) * density, gradient));
    vec4 tex_color = texture(texture_map, vec3(uv_wrap(v_uv), float(v_tex_id)));
    // return ((n-start1)/(stop1-start1))*(stop2-start2)+start2;
    float ao = pow(v_ao, 1.0 / AO_CURVE) * (1.0 - MIN_AO) + MIN_AO;
    vec4 col = vec4(v_face_scalar * ao * ambient_light, 1.0) * tex_color;

    color = col; //mix(vec4(0.729411765, 0.907843137, 0.981568627, 1.0), col, fog);
}
