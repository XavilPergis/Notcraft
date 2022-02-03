#pragma include "./fullscreen_quad.vert"

#pragma shaderstage fragment
#version 330 core

#pragma include "/util.glsl"
#pragma include "/noise.glsl"
#pragma include "/adjustables.glsl"

uniform sampler2D colorBuffer;
uniform sampler2D depthBuffer;

uniform uvec2 screenDimensions;
uniform vec3 cameraPosWorld;
uniform mat4 viewMatrix;
uniform mat4 projectionMatrix;

uniform uint elapsedSeconds;
uniform float elapsedSubseconds;

float elapsedTime() {
    return float(elapsedSeconds) + elapsedSubseconds;
}

in vec2 v_texcoord;
in vec2 v_uv;
out vec4 o_color;

float fogFactorExp(float density, float t) {
    return clamp(1.0 - exp(-density * t), 0.0, 1.0);
}

float fogFactorExp2(float density, float t) {
    float n = density * t;
    return clamp(1.0 - exp(-(n * n)), 0.0, 1.0);
}

const highp float NOISE_GRANULARITY = 0.2/255.0;

void main() {
    vec3 originalColor = texture2D(colorBuffer, v_texcoord).rgb;
    vec3 color = originalColor;
    float depth = 2.0 * texture2D(depthBuffer, v_texcoord).r - 1.0;

    vec2 uvClip = v_uv;
    
    vec4 clipPos = vec4(uvClip, depth, 1.0);
    vec4 viewPos = inverse(projectionMatrix) * clipPos;
    viewPos /= viewPos.w;

    vec3 worldPos = (inverse(viewMatrix) * viewPos).xyz;

    float distToSurface = length(worldPos - cameraPosWorld) / DAY_NIGHT(900.0, 900.0);
    // float fogStrength = fogFactorExp(0.4, distToSurface);
    float fogStrength = 0.0;

    vec3 fogColor = DAY_NIGHT(FOG_COLOR, FOG_COLOR_NIGHT);
    vec3 finalColor = mix(color, fogColor, fogStrength);
    finalColor += mix(-NOISE_GRANULARITY, NOISE_GRANULARITY, random(vec2(v_texcoord.x, v_texcoord.y + elapsedSubseconds)));
    
    o_color = vec4(finalColor, 1.0);
}
