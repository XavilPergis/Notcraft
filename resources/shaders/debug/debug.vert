#version 330 core

uniform mat4 view;
uniform mat4 projection;

in vec3 pos;
in vec4 color;
// line type + one bit for marking either the start (false) or end (true) of the line.
in uint kind_end;

out vec4 v_color;
out float v_end;
flat out int v_kind;

void main() {
    gl_Position = projection * view * vec4(pos, 1.0);
    
    v_color = color;
    v_kind = int(kind_end) >> 1;
    v_end = float(int(kind_end) & 1);
}
