#version 330 core

layout (location = 0) in vec3 pos;
layout (location = 1) in vec2 uv;

out vec2 uv_varying;

uniform vec2 resolution;

void main() {
    vec2 res = resolution / resolution.y;
    // vec2 uv = (fragCoord.xy - iResolution.xy * .5) / iResolution.y;
    gl_Position = vec4(vec3(pos.x / res.x, pos.y, pos.z) / 100.0, 1.0);
    uv_varying = uv;
}
