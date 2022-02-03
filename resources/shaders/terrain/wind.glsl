#pragma include "/noise.glsl"
#pragma include "/adjustables.glsl"

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

struct NoiseParameters {
    int octaveCount;
    float baseFrequency;
    float frequencyScale;
    float baseAmplitude;
    float amplitudeScale;
    float scrollSpeed;
    float bubbleSpeed;
};

float calculateNoise(NoiseParameters params, vec3 worldPos, float t) {
    float frequency = params.baseFrequency;
    float amplitude = params.baseAmplitude;
    float total = 0.0;
    float maxTotal = 0.0;

    for (int octave = 0; octave < params.octaveCount; ++octave) {
        vec3 motionDirection = vec3(0.0, 0.0, 0.0);
        motionDirection.x = random(float(octave) / float(params.octaveCount));
        motionDirection.z = random(float(octave) / float(params.octaveCount));
        motionDirection.y = params.bubbleSpeed / float(octave + 1);

        vec3 scrollOffset = params.scrollSpeed * t * motionDirection;
        vec3 pos = worldPos + scrollOffset;

        // can almost think of this as a map of wind pockets that scroll over the landscape over time
        total += amplitude * simplexNoise(frequency * pos);
        maxTotal += amplitude;

        frequency *= params.frequencyScale;
        amplitude *= params.amplitudeScale;
    }

    return total / maxTotal;
}

float cloudDensity(vec3 worldPos, float t) {
    NoiseParameters params = CLOUD_NOISE_PARAMS;
    float densityNoise = calculateNoise(params, worldPos, t);
    
    vec3 coverageOffset = vec3(0.0, 0.0, 0.0);
    coverageOffset.xz = CLOUD_NOISE_COVERAGE_SCROLL_DIRECTION;
    coverageOffset.xz *= CLOUD_NOISE_COVERAGE_SCROLL_SPEED * t;
    float coverageNoise = simplexNoise(CLOUD_NOISE_COVERAGE_SCALE * (worldPos + vec3(coverageOffset.x, 0.0, coverageOffset.y)));
    coverageNoise = smoothstep(-1.0, 1.0, coverageNoise);
    // return coverageNoise;
    
    return max(1.0 - coverageNoise, 0.5 * densityNoise + 0.5) - (1.0 - coverageNoise);
}

float windIntensity(vec3 worldPos, float t) {
    return max(0.0, cloudDensity(worldPos, t));
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
