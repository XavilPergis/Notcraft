#version 330 core

in vec4 v_color;
in float v_end;
flat in int v_kind;

out vec4 o_color;

#define DASHES_PER_UNIT (20.0)
#define DOTS_PER_UNIT (50.0)

#define KIND_SOLID_LINE 0
#define KIND_DASHED_LINE 1
#define KIND_DOTTED_LINE 2

float dotted_factor(float end_percent) {
    float t = end_percent * DOTS_PER_UNIT;
    return floor(2.0 * t) - 2.0 * floor(t);
}

float dashed_factor(float end_percent) {
    float t = end_percent * DASHES_PER_UNIT;
    return floor(2.0 * t) - 2.0 * floor(t);
}

void main() {
    vec4 color = v_color.rgba;
    switch (v_kind) {
        case KIND_SOLID_LINE: break;
        case KIND_DASHED_LINE: color.a *= dashed_factor(v_end); break;
        case KIND_DOTTED_LINE: color.a *= dotted_factor(v_end); break;
    }
    o_color = color;
}
