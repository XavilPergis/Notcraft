#version 330 core

uniform sampler2D positions;
uniform sampler2D normals;
uniform sampler2D colors;
uniform sampler2D extra;

uniform vec3 eye;
uniform vec3 sun_dir;

in vec2 v_pos;
out vec4 final_color;

#define SPECULAR_POWER 50.0
#define SUN_COLOR vec4(1.0, 0.7, 0.8, 1.0)

// float diffuse() {
    
// }

vec4 compose(vec2 uv) {
    
    vec4 color = texture(colors, uv);
    vec3 pos = texture(positions, uv).xyz;
    vec3 normal = texture(normals, uv).xyz;

    vec3 dir = normalize(sun_dir);
    vec3 to_camera = normalize(eye - pos);

    float diffuse_intensity = dot(normal, dir);
    vec4 diffuse = max(diffuse_intensity, 0.0) * SUN_COLOR;

    float specular_dot = dot(reflect(-dir, normal), to_camera);
    float specular_intensity = pow(max(specular_dot, 0.0), SPECULAR_POWER);
    vec4 specular = specular_intensity * SUN_COLOR;

    return diffuse * color + specular;
}

void main() {
    if (v_pos.x > 0.0 && v_pos.y < 0.0) {
        final_color = texture(colors, v_pos);
    }

    if (v_pos.x > 0.0 && v_pos.y > 0.0) {
        final_color = texture(normals, v_pos);
    }

    if (v_pos.x < 0.0 && v_pos.y < 0.0) {
        final_color = texture(positions, v_pos);
    }

    if (v_pos.x < 0.0 && v_pos.y > 0.0) {
        final_color = texture(extra, v_pos);
        //vec2 uv = 0.5 * v_pos - vec2(0.5, 0.5);
        // vec2 uv = v_pos;
        // final_color = compose(uv);
    }
}
