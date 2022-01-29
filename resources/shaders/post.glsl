#pragma include "./fullscreen_quad.vert"

#pragma shaderstage fragment
#version 330 core

#pragma include "/util.glsl"

uniform sampler2D b_color;
uniform sampler2D b_depth;

in vec2 v_texcoord;
out vec4 o_color;

void main() {
    vec3 originalColor = texture2D(b_color, v_texcoord).rgb;
    vec3 color = originalColor;
    float depth = texture2D(b_depth, v_texcoord).r;

    // 0.1, 1000.0
    float distToSurface = (1.0 - depth) * 10000.0;
    // float fogStrength = exp(pow(-10.0 * distToSurface, 2.0));
    float fogStrength = exp(-pow(0.3 * distToSurface, 1.1));

    vec3 finalColor = mix(color, mix(RGB(148, 197, 252), RGB(55, 127, 252), 0.5), fogStrength);

    if (depth >= 0.99999) {
        finalColor = originalColor;
        // finalColor.r = 1.0;
        // finalColor.g = 0.0;
        // finalColor.b = 0.0;
    }

    o_color = vec4(finalColor, 1.0);
}
