#version 330 core

uniform vec3 camera_position;
uniform vec3 ambient_light;
uniform sampler2DArray texture_map;

in vec3 v_pos;
in vec3 v_normal;
in vec3 v_face_scalar;
in vec2 v_uv;
flat in int v_tex_id;

out vec4 color;

void main()
{
    float density = 0.007;
    float gradient = 5.0;
    float fog = exp(-pow(length(camera_position - v_pos) * density, gradient));
    vec4 tex_color = texture(texture_map, vec3(v_uv, float(v_tex_id)));
    // return ((n-start1)/(stop1-start1))*(stop2-start2)+start2;
    vec4 col = vec4(v_face_scalar * ambient_light, 1.0) * tex_color;

    color = mix(vec4(0.729411765, 0.907843137, 0.981568627, 1.0), col, fog);
}
