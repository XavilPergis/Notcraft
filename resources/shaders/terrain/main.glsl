#pragma include "unpack.glsl"

#pragma shaderstage vertex
#pragma include "wind.glsl"
#pragma include "/util.glsl"

uniform mat4 model;
uniform mat4 view;
uniform mat4 projection;

out float vSkyLight;
out float vBlockLight;
out float vStaticBrightness;
out vec3 vWorldPos;
out vec2 vTextureUv;
flat out int vTextureId;

#define AO_MIN_BRIGHTNESS 0.3
#define AO_ATTENUATION 1.5

float contribution(bool contribute, float strength) {
    return 1.0 - float(contribute) * strength;
}

uniform uint elapsedSeconds;
uniform float elapsedSubseconds;

float elapsedTime() {
    return 2.0 * (float(elapsedSeconds) + elapsedSubseconds);
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

    vBlockLight = vertex.blockLight;
    vSkyLight = vertex.skyLight;
    vStaticBrightness = brightness;

    vTextureUv = vertex.textureCoordinates;
    vTextureId = vertex.textureId;
}

#pragma shaderstage fragment
#version 330 core

#pragma include "wind.glsl"
#pragma include "/adjustables.glsl"

uniform sampler2DArray albedo_maps;

uniform uint elapsedSeconds;
uniform float elapsedSubseconds;

float elapsedTime() {
    return float(elapsedSeconds) + elapsedSubseconds;
}

in float vStaticBrightness;
in float vBlockLight;
in float vSkyLight;
in vec2 vTextureUv;
flat in int vTextureId;
in vec3 vWorldPos;

out vec3 b_color;

// const highp float NOISE_GRANULARITY = 0.2/255.0;

// higher values mean light "drops off" from the source more quickly.
#define LIGHT_ATTENUATION 2.0

// lower values mean that zero brightness is darker.
#define LIGHT_MIN_BRIGHNESS 0.04

void main() {
    vec4 fragmentColor = texture(albedo_maps, vec3(vTextureUv, vTextureId));
    if (fragmentColor.a < 0.5) {
        discard;
    }

    float cloudFactor = 1.0 - smoothstep(0.15, 0.4, cloudDensity(vec3(vWorldPos.x, 1000.0, vWorldPos.z), elapsedTime()));
    cloudFactor = mix(0.3, 1.0, pow(cloudFactor, 8.0));
    cloudFactor = mix(1.0, cloudFactor, vSkyLight); // [min, 1]

    float dayNightFactor = DAY_NIGHT_FACTOR(elapsedTime()); // [0, 1]

    float skyLightFactor = mix(LIGHT_MIN_BRIGHNESS, 1.0, pow(vSkyLight * DAY_NIGHT_FACTOR(elapsedTime()), LIGHT_ATTENUATION)); // [min, skyLight]
    float blockLightFactor = mix(LIGHT_MIN_BRIGHNESS, 1.0, pow(vBlockLight, LIGHT_ATTENUATION)); // [min, blockLight]

    float brightness = 0.0;

    // [bmin, blockLight]
    brightness = max(brightness, skyLightFactor);
    brightness *= cloudFactor;
    brightness = max(brightness, blockLightFactor);

    brightness *= vStaticBrightness;

    // if (cloudFactor > 0.001 && cloudFactor < 0.005) {
    //     fragmentColor.rgb += vec3(1.0);
    // } else if (cloudFactor > 0.401 && cloudFactor < 0.405) {
    //     fragmentColor.r = 1.0;
    // } else if (cloudFactor > 0.405) {
    //     fragmentColor.r += 0.01;
    // } else if (cloudFactor > 0.005) {
    //     fragmentColor.rgb += vec3(0.01);
    // } 
    // fragmentColor.rgb = vec3(cloudFactor);
    fragmentColor.rgb *= brightness;
    // fragmentColor.rgb = vec3(vTextureUv / 32.0, 1.0);

    // apply some slight noise to mitigate banding in dark regions.
    // fragmentColor += mix(-NOISE_GRANULARITY, NOISE_GRANULARITY, random(vTextureUv));

    b_color = fragmentColor.rgb;
}
