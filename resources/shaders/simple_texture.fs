#version 330 core

out vec4 color;
in vec2 uv_varying;

uniform sampler2D tex;

void main() {
    vec4 sample = texture(tex, uv_varying);
    if (sample.a == 0.0) discard;
    color = sample;
}
