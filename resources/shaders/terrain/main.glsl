#pragma include "unpack.glsl"

#pragma shaderstage vertex
#pragma include "wind.glsl"
#pragma include "/util.glsl"

uniform mat4 model;
uniform mat4 view;
uniform mat4 projection;

out vec3 vColorFilter;
out vec3 vWorldPos;
out vec2 vTextureUv;
flat out int vTextureId;

float contribution(bool contribute, float strength) {
    return 1.0 - float(contribute) * strength;
}

// higher values mean light "drops off" from the source more quickly.
#define LIGHT_ATTENUATION 2.0

// lower values mean that zero brightness is darker.
#define LIGHT_MIN_BRIGHNESS 0.03

#define AO_MIN_BRIGHTNESS 0.3
#define AO_ATTENUATION 1.5

uniform uint elapsed_seconds;
uniform float subseconds;

float elapsedTime() {
    return 2.0 * (float(elapsed_seconds) + subseconds);
}

void main() {
    TerrainVertex vertex = unpackVertex();

    vec3 worldPos = (model * vec4(vertex.modelPos, 1.0)).xyz;
    vWorldPos = worldPos;

    if (vertex.windSway) {
        worldPos.xz += windTotal(worldPos, elapsedTime());
    }

    gl_Position = projection * view * vec4(worldPos, 1.0);

    float brightness = 1.0;

    float aoFactor = 1.0;
    aoFactor *= mix(AO_MIN_BRIGHTNESS, 1.0, vertex.ao);
    aoFactor = pow(aoFactor, AO_ATTENUATION);
    brightness *= aoFactor;

    // directional lighting
    float directionFactor = 1.0;
    directionFactor *= contribution(vertex.axis == AXIS_X, 0.3); // X
    directionFactor *= contribution(vertex.axis == AXIS_Y && vertex.axisSign == SIGN_POSITIVE, 0.0); // top
    directionFactor *= contribution(vertex.axis == AXIS_Y && vertex.axisSign == SIGN_NEGATIVE, 0.5); // bottom
    directionFactor *= contribution(vertex.axis == AXIS_Z, 0.4); // Z
    brightness *= directionFactor;

    float lightFactor = max(vertex.blockLight, vertex.skyLight);
    lightFactor = pow(lightFactor, LIGHT_ATTENUATION);
    lightFactor = mix(LIGHT_MIN_BRIGHNESS, 1.0, lightFactor);
    brightness *= lightFactor;

    vColorFilter = vec3(brightness);

    vTextureUv = vertex.textureCoordinates;
    vTextureId = vertex.textureId;
}

#pragma shaderstage fragment
#version 330 core

#pragma include "wind.glsl"

uniform sampler2DArray albedo_maps;

uniform uint elapsed_seconds;
uniform float subseconds;

float elapsedTime() {
    return float(elapsed_seconds) + subseconds;
}

in vec3 vColorFilter;
in vec2 vTextureUv;
flat in int vTextureId;
in vec3 vWorldPos;

out vec3 b_color;

const highp float NOISE_GRANULARITY = 0.2/255.0;

void main()
{
    vec4 fragmentColor = texture(albedo_maps, vec3(vTextureUv, vTextureId));
    if (fragmentColor.a < 0.5) {
        discard;
    }

    // float windIntensity = max(0.0, simplexNoise(0.001 * vWorldPos.xz + 0.01 * vec2(1.0, 0.1) * elapsedTime()));
    float windIntensity = windIntensity(vWorldPos, elapsedTime());

    // fragmentColor.rgb = vec3(0.0);
    // fragmentColor.r = subseconds;
    // fragmentColor.g = mod(float(elapsed_seconds), 10.0) / 10.0;
    // fragmentColor.b = mod(elapsedTime(), 10.0) / 10.0;
    fragmentColor.rgb *= mix(1.0, 0.4, windIntensity);
    // fragmentColor.rgb += windIntensity;
    fragmentColor.rgb *= vColorFilter;
    // fragmentColor.rgb = vec3(vTextureUv / 32.0, 1.0);
    // fragmentColor.rgb = vec3(valueNoise2d(0.1 * vTextureUv));

    // apply some slight noise to mitigate banding in dark regions.
    fragmentColor += mix(-NOISE_GRANULARITY, NOISE_GRANULARITY, random(vTextureUv));

    b_color = fragmentColor.rgb;
}
