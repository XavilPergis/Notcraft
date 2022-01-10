#pragma shaderstage vertex
#version 330 core

in uint pos_ao;
in uint light_side_id;

uniform mat4 model;
uniform mat4 view;
uniform mat4 projection;

out vec3 vColorFilter;
out vec3 vNormal;
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

#define BITS(packed, a, n) ((packed) >> (a)) & uint((1 << (n)) - 1)

void main()
{
    vec3 normalFromAxis[3];
    float axisSign[2];

    normalFromAxis = vec3[3](
        vec3(1.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        vec3(0.0, 0.0, 1.0)
    );
    axisSign = float[2](1.0, -1.0);

    // unpack attributes
    float ao = float(BITS(pos_ao, 0, 2)) / 3.0;

    float z = float(BITS(pos_ao, 2, 10)) / 16.0;
    float y = float(BITS(pos_ao, 12, 10)) / 16.0;
    float x = float(BITS(pos_ao, 22, 10)) / 16.0;
    vec3 pos = vec3(x, y, z);

    int id = int(BITS(light_side_id, 0, 16));
    uint baxis = uint(BITS(light_side_id, 16, 2));
    uint baxisSign = uint(BITS(light_side_id, 18, 1));
    float blockLight = float(BITS(light_side_id, 24, 4)) / 16.0;
    float skyLight = float(BITS(light_side_id, 28, 4)) / 16.0;

    vec3 normal = normalFromAxis[baxis];
    normal *= axisSign[baxisSign];

    vec2 uvFromAxis[3];
    uvFromAxis = vec2[3](
        vec2(pos.z, pos.y),
        vec2(pos.x, pos.z),
        vec2(pos.x, pos.y)
    );
    vec2 uv = uvFromAxis[baxis];

    // normal shader code
    gl_Position = projection * view * model * vec4(pos, 1.0);

    float brightness = 1.0;

    float aoFactor = 1.0;
    aoFactor *= mix(AO_MIN_BRIGHTNESS, 1.0, ao);
    aoFactor = pow(aoFactor, AO_ATTENUATION);
    brightness *= aoFactor;

    // directional lighting
    float directionFactor = 1.0;
    directionFactor *= contribution(baxis == uint(1) && baxisSign == uint(0), 0.0); // top
    directionFactor *= contribution(baxis == uint(1) && baxisSign == uint(1), 0.5); // bottom
    directionFactor *= contribution(baxis == uint(0), 0.3); // X
    directionFactor *= contribution(baxis == uint(2), 0.4); // Z
    brightness *= directionFactor;

    float lightFactor = max(blockLight, skyLight);
    lightFactor = pow(lightFactor, LIGHT_ATTENUATION);
    lightFactor = mix(LIGHT_MIN_BRIGHNESS, 1.0, lightFactor);
    brightness *= lightFactor;

    vColorFilter = vec3(brightness);

    vTextureId = id;
    vTextureUv = uv;
    vNormal = normal;
}

#pragma shaderstage fragment
#version 330 core

uniform sampler2DArray albedo_maps;

in vec3 vColorFilter;
in vec2 vTextureUv;
flat in int vTextureId;

out vec3 b_color;

const highp float NOISE_GRANULARITY = 0.2/255.0;

highp float random(vec2 coords) {
   return fract(sin(dot(coords.xy, vec2(12.9898,78.233))) * 43758.5453);
}

void main()
{
    vec4 fragmentColor = texture(albedo_maps, vec3(vTextureUv, vTextureId));
    if (fragmentColor.a < 0.5) {
        discard;
    }

    fragmentColor.rgb *= vColorFilter;

    // apply some slight noise to mitigate banding in dark regions.
    fragmentColor += mix(-NOISE_GRANULARITY, NOISE_GRANULARITY, random(vTextureUv));

    b_color = fragmentColor.rgb;
}
