#version 330 core

in vec2 uv;

out vec2 v_texcoord;

void main() {
    v_texcoord = 0.5 * uv + 0.5;
    gl_Position = vec4(uv, 0.0, 1.0);
}
