
#pragma include "./fullscreen_quad.vert"

#pragma shaderstage fragment
#version 330 core

#pragma include "terrain/wind.glsl"
#pragma include "/adjustables.glsl"

uniform uvec2 screenDimensions;
uniform vec3 cameraPosWorld;
uniform mat4 projectionMatrix;
uniform mat4 viewMatrix;

uniform uint elapsedSeconds;
uniform float elapsedSubseconds;

float elapsedTime() {
    return float(elapsedSeconds) + elapsedSubseconds;
}

in vec2 v_texcoord;
in vec2 v_uv;

out vec4 b_color;

#define UP (vec3(0.0, 1.0, 0.0))

struct Intersection {
    bool intersects;
    vec3 point;
};

Intersection rayPlaneIntersection(vec3 rayOrigin, vec3 rayDir, vec3 planeNormal, float planeDistance) {
    // t = -(O . N - d) / (V . N)
    // P = O + -((O . N - d) / (V . N))V
    float t = (-dot(rayOrigin, planeNormal) + planeDistance) / dot(rayDir, planeNormal);
    if (t >= 0.0) {
        return Intersection(true, rayOrigin + t * rayDir);
    }

    return Intersection(false, vec3(0.0, 0.0, 0.0));
}

float densityAt(vec3 v) {
    // v.xz /= 4.0;
    v.y -= cameraPosWorld.y;
    return max(0.0, cloudDensity(v, elapsedTime()));
}

void main() {
    vec3 intoScreen =  -vec3(-v_uv, 1.0);
    intoScreen.x *= float(screenDimensions.x) / float(screenDimensions.y);
    // intoScreen *= 2.0;
    // intoScreen = fract(intoScreen);
    vec4 rayDirWorld = inverse(viewMatrix) * normalize(vec4(intoScreen, 0.0));

    float downCloseness = pow(dot(rayDirWorld.xyz, UP), DAY_NIGHT(0.5, 0.2));

    vec3 dayColor = mix(SKY_COLOR_BRIGHT, SKY_COLOR_BASE, max(0.0, downCloseness));
    vec3 nightColor = mix(SKY_COLOR_NIGHT_BRIGHT, SKY_COLOR_NIGHT_BASE, max(0.0, downCloseness));
    vec3 color = DAY_NIGHT(dayColor, nightColor);


    Intersection p = rayPlaneIntersection(cameraPosWorld.xyz, rayDirWorld.xyz, UP, cameraPosWorld.y + CLOUD_PLANE_DISTANCE);

    float distanceToEdge = length(p.point.xz - cameraPosWorld.xz);
    if (p.intersects && distanceToEdge <= CLOUD_PLANE_DISTANCE_CUTOFF) {
        float cloudFactor = 1.0;
        for (int i = 0; i < 1; ++i) {
            vec3 v = p.point + float(350 * i) * rayDirWorld.xyz;
            vec3 u = p.point + float(350 * (i + 1)) * rayDirWorld.xyz;

            float cloudIntensity = densityAt(v);
            cloudIntensity *= 1.0 - (distanceToEdge / CLOUD_PLANE_DISTANCE_CUTOFF);

            cloudFactor *= cloudIntensity;
        }

        vec3 cloudBaseDay = vec3(1.0);
        vec3 cloudBaseNight = 3.5 * SKY_COLOR_NIGHT_BASE;
        vec3 cloudBase = DAY_NIGHT(cloudBaseDay, cloudBaseNight);

        vec3 cloudIntenseDay = 0.9 * color;
        vec3 cloudIntenseNight = 1.5 * SKY_COLOR_NIGHT_BASE;
        vec3 cloudIntense = DAY_NIGHT(cloudIntenseDay, cloudIntenseNight);

        float intenseFactor = smoothstep(0.2, 1.0, 1.0 - pow(1.0 - cloudFactor, 2.1));
        vec3 cloudColor = mix(cloudBase, cloudIntense, intenseFactor);
        cloudFactor = smoothstep(0.15, 0.2, cloudFactor);

        color = mix(color, cloudColor, cloudFactor);
    }

    b_color = vec4(color, 1.0);
}
