#pragma shaderstage vertex
#version 330 core

uniform float screen_width;
uniform float screen_height;

in vec2 uv;
out vec2 v_texcoord;

void main() {
    vec2 pos = uv;
    pos.x /= (screen_width / screen_height);
    pos /= screen_height / 16.0;

    v_texcoord = 0.5 * uv + 0.5;
    gl_Position = vec4(pos, 0.0, 1.0);
}

#pragma shaderstage fragment
#version 330 core

uniform sampler2D crosshair_texture;

in vec2 v_texcoord;
out vec4 o_color;

void main() {
    o_color = texture2D(crosshair_texture, v_texcoord);
}
