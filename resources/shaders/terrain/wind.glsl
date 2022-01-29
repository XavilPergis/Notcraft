#pragma include "/noise.glsl"

#define WIND_INTENSITY_SCROLL_DIRECTION vec2(1.0, 0.1)
#define WIND_INTENSITY_SCROLL_SPEED 3.0

vec2 windDirection(vec3 worldPos, float t) {
    vec2 windDirection = vec2(0.0, 0.0);
    windDirection.x = simplexNoise(0.001 * vec3(worldPos.xz, t));
    windDirection.y = simplexNoise(0.001 * vec3(t, worldPos.xz));
    return windDirection;
}

float remap01(float t) {
    return 0.5 * t + 0.5;
}

float windIntensity(vec3 worldPos, float t) {
    vec2 pos = vec2(0.0, 0.0);

    // can almost think of this as a map of wind pockets that scroll over the landscape over time
    pos = worldPos.xz + 0.4 * t * vec2(0.2, 0.5);
    float f0 = 1.0 * simplexNoise(0.001 * pos);

    pos = worldPos.xz + 1.5 * t * vec2(1.0, 0.1);
    float f1 = 0.7 * simplexNoise(0.01 * pos);
    
    pos = worldPos.xz + 3.0 * t * -vec2(0.2, 0.5);
    float f2 = 0.15 * simplexNoise(0.07 * pos);
    
    pos = worldPos.xz + 2.0 * t * vec2(-1.0, 0.3);
    float f3 = 0.075 * simplexNoise(0.4 * pos);

    return max(0.0, f0 + f1 + f2 + f3);
}

float windLocalStrength(vec3 worldPos, float t) {
    return 0.5 + 0.5 * simplexNoise(vec2(worldPos.x + t, worldPos.z - 0.5 * t));
}

vec2 windTotal(vec3 worldPos, float t) {
    vec2 direction = windDirection(worldPos, t);
    float localStrength = windLocalStrength(worldPos, t);
    float intensity = windIntensity(worldPos, t);

    return mix(0.1, 1.0, intensity) * (localStrength * direction);
}
