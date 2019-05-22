#version 330 core

#define BIAS 0.00025
#define SAMPLE_COUNT 64
#define SAMPLE_RADIUS (1.0/3.0)

uniform sampler2D gdepth;
uniform sampler2D gnormal;
uniform sampler2D gposition;

// uniform sampler2D ao_noise;
uniform samples {
    vec4 sample_array[SAMPLE_COUNT];
};
uniform mat4 view_matrix;
uniform mat4 projection_matrix;

in vec2 texcoord;
out float ao;

float mod289(float x){return x - floor(x * (1.0 / 289.0)) * 289.0;}
vec4 mod289(vec4 x){return x - floor(x * (1.0 / 289.0)) * 289.0;}
vec4 perm(vec4 x){return mod289(((x * 34.0) + 1.0) * x);}

float noise(vec3 p){
    vec3 a = floor(p);
    vec3 d = p - a;
    d = d * d * (3.0 - 2.0 * d);

    vec4 b = a.xxyy + vec4(0.0, 1.0, 0.0, 1.0);
    vec4 k1 = perm(b.xyxy);
    vec4 k2 = perm(k1.xyxy + b.zzww);

    vec4 c = k2 + a.zzzz;
    vec4 k3 = perm(c);
    vec4 k4 = perm(c + 1.0);

    vec4 o1 = fract(k3 * (1.0 / 41.0));
    vec4 o2 = fract(k4 * (1.0 / 41.0));

    vec4 o3 = o2 * d.z + o1 * (1.0 - d.z);
    vec2 o4 = o3.yw * d.x + o3.xz * (1.0 - d.x);

    return o4.y * d.y + o4.x * (1.0 - d.y);
}

vec3 random_vec(vec2 v) {
    return vec3(noise(vec3(100.0 * v, 0.0)), noise(vec3(100.0 * v, 0.33)), 0.0);
}

vec3 world_to_view(vec3 wpos) {
    vec4 pos = view_matrix * vec4(wpos, 1.0);
    return pos.xyz;
}

vec3 world_to_view(vec4 wpos) {
    return world_to_view(wpos.xyz);
}

void main() {
    vec3 normal_world = texture2D(gnormal, texcoord).xyz;
    if (length(normal_world) == 0) {
        ao = 1.0;
        return;
    }
    vec3 pos = texture2D(gposition, texcoord).xyz;
    vec3 normal = normalize(normal_world);

    vec3 random_vec = random_vec(texcoord);
    vec3 tangent = normalize(random_vec - normal * dot(random_vec, normal));
    vec3 bitangent = cross(normal, tangent);
    mat3 tbn = mat3(tangent, bitangent, normal);

    float occlusion = 0.0;
    for (int i = 0; i < SAMPLE_COUNT; ++i) {
        // Multiplying by the tbn matrix *rotates* from tangent space to view space, but it's still at the origin,
        // so we add pos to correct ourselves.
        vec3 sample = tbn * sample_array[i].xyz;
        sample = pos + sample * SAMPLE_RADIUS;
        sample = world_to_view(sample);

        // Get texture coordiantes
        vec4 offset = vec4(sample, 1.0);
        offset = projection_matrix * offset;
        offset.xyz /= offset.w;
        offset.xyz = 0.5 * offset.xyz + 0.5;
        
        // Don't have the sample contribute if it falls in an invalid range
        float invalid_check = length(texture2D(gnormal, offset.xy).xyz);
        
        vec3 actual_pos = texture2D(gposition, offset.xy).xyz;
        actual_pos = world_to_view(actual_pos);

        float range_check = 1.0; smoothstep(0.0, 1.0, SAMPLE_RADIUS / abs(world_to_view(pos).z - actual_pos.z));
        occlusion += invalid_check * range_check * (actual_pos.z >= sample.z + BIAS ? 1.0 : 0.0);
    }
    ao = 1.0 - occlusion / SAMPLE_COUNT;
}
