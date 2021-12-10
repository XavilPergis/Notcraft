#version 330 core

uniform sampler2D b_color;
uniform sampler2D b_depth;

in vec2 v_texcoord;

out vec4 o_color;

void main() {
    vec3 color = texture2D(b_color, v_texcoord).rgb;
    o_color = vec4(color, 1.0);
}
