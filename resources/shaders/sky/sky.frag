#version 330 core

// uniform vec3 camera_pos;
uniform mat4 projection_matrix;
uniform mat4 view_matrix;
uniform mat4 inverse_view_matrix;

in vec2 v_texcoord;
in vec2 v_uv;

out vec4 b_color;

#define UP (vec3(0.0, 1.0, 0.0))

#define RGB(r, g, b) (vec3(r.0 / 255.0, g.0 / 255.0, b.0 / 255.0))

#define SKY_COLOR_BASE RGB(55, 147, 252)
#define SKY_COLOR_BRIGHT RGB(148, 197, 252)

void main() {
    vec3 into_screen =  -vec3(-v_uv, 1.0);
    vec4 ray_dir_world = inverse(view_matrix) * normalize(vec4(into_screen, 0.0));

    float up_closeness = pow(dot(ray_dir_world.xyz, UP), 0.7);
    vec3 color = mix(SKY_COLOR_BASE, SKY_COLOR_BRIGHT, max(0.0, up_closeness));

    b_color = vec4(color, 1.0);
}
