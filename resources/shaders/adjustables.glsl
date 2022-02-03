#define RGB(r, g, b) (vec3(r.0 / 255.0, g.0 / 255.0, b.0 / 255.0))

#define SKY_COLOR_BASE RGB(55, 127, 252)
#define SKY_COLOR_BRIGHT RGB(148, 197, 252)
#define SKY_COLOR_NIGHT_BASE RGB(0, 0, 0)
#define SKY_COLOR_NIGHT_BRIGHT RGB(1, 1, 1)

#define FOG_COLOR mix(SKY_COLOR_BASE, SKY_COLOR_BRIGHT, 0.5)
#define FOG_COLOR_NIGHT mix(SKY_COLOR_NIGHT_BASE, SKY_COLOR_NIGHT_BRIGHT, 0.5)

#define DAY_NIGHT_LENGTH 5.0
#define _DAY_NIGHT_M1_1(time) sin(3.14159 * time / DAY_NIGHT_LENGTH)
#define _DAY_NIGHT_0_1(time) 0.5 * _DAY_NIGHT_M1_1(time) + 0.5
// #define DAY_NIGHT_FACTOR(time) smoothstep(-0.4, 0.4, _DAY_NIGHT_M1_1(time))
#define DAY_NIGHT_FACTOR(time) 1.0
#define DAY_NIGHT(day, night) mix(night, day, DAY_NIGHT_FACTOR(elapsedTime()))

// #define CLOUD_PLANE_DISTANCE 10000.0
#define CLOUD_PLANE_DISTANCE 1000.0
#define CLOUD_PLANE_DISTANCE_CUTOFF 10000.0

// IDEA: use voroni noise for clouds, may end up looking much smoother and nicer that way, and less cotton-ball-like
// IDEA: noise map to control cloud coverage
#define CLOUD_NOISE_PARAMS NoiseParameters( \
        /* octaveCount    */ 5,     \
        /* baseFrequency  */ 0.001, \
        /* frequencyScale */ 3.5,   \
        /* baseAmplitude  */ 1.0,   \
        /* amplitudeScale */ 0.3,   \
        /* scrollSpeed    */ 4.0,   \
        /* bubbleSpeed    */ 1.0    \
    );

#define CLOUD_NOISE_COVERAGE_SCALE 0.0001
#define CLOUD_NOISE_COVERAGE_SCROLL_SPEED 20.0
#define CLOUD_NOISE_COVERAGE_SCROLL_DIRECTION vec2(0.8, 0.2)
