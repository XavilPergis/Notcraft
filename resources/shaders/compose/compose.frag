#version 330 core

#define NUM_LIGHTS 64
#define PI 3.1415926535

uniform sampler2D gposition;
uniform sampler2D gcolor;
uniform sampler2D gnormal;
uniform sampler2D gemissive;
uniform sampler2D gdepth;
uniform sampler2D gextra;
uniform sampler2D ao;

uniform vec3 camera_pos;
uniform mat4 projection_matrix;
uniform mat4 view_matrix;

in vec2 texcoord;

out vec4 color;

uniform light_position_buffer {
    vec4 light_positions[NUM_LIGHTS];
};

uniform light_color_buffer {
    vec4 light_colors[NUM_LIGHTS];
};

float geometry_schlick_ggx(float NdotV, float roughness) {
    float r = (roughness + 1.0);
    float k = (r*r) / 8.0;

    float num   = NdotV;
    float denom = NdotV * (1.0 - k) + k;
	
    return num / denom;
}

// Smith
float brdf_geometry_term(vec3 N, vec3 V, vec3 L, float roughness) {
    float NdotV = max(dot(N, V), 0.0);
    float NdotL = max(dot(N, L), 0.0);
    float ggx2  = geometry_schlick_ggx(NdotV, roughness);
    float ggx1  = geometry_schlick_ggx(NdotL, roughness);
	
    return ggx1 * ggx2;
}

vec3 brdf_fresnel_term(float NdotV, vec3 F0) {
    return F0 + (1 - F0) * pow(1 - NdotV, 5);
}

float brdf_distribution_term(vec3 N, vec3 H, float roughness) {
    float a = roughness*roughness;
    float a2 = a*a;
    float NdotH  = max(dot(N, H), 0.0);
    float NdotH2 = NdotH*NdotH;
	
    float nom = a2;
    float denom = (NdotH2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;
	
    return nom / denom;
}

// vec3 brdf(vec3 N, vec3 Wo, vec3 N, float a) {

// }

// vec3 radiance()


// vec3 uv_to_ndc(vec2 texcoord) {
//     float depth = texture2D(gdepth, texcoord).r;
//     return 2 * vec3(texcoord, depth) - 1;
// }

// vec3 uv_to_world(vec2 texcoord) {
//     // Reconstruct clip space coordinates from fullscreen quad. (quad -> clip)
//     vec3 ndc = uv_to_ndc(texcoord);
//     // clip -> view
//     vec4 view = inverse(projection_matrix) * vec4(ndc, 1);
//     view /= view.w;
//     // view -> world
//     return inverse(view_matrix) * view;
// }

// vec3 incoming_radiance(vec3 dir) {
// }

mat3 rotate_y(float theta) {
    return mat3(
          vec3(cos(theta), 0.0, -sin(theta)),
        vec3(0.0,        1.0, 0.0),
          vec3(sin(theta), 0.0, cos(theta))
    );
}

void main() {
    vec3 world_pos = texture2D(gposition, texcoord).xyz;
    vec3 albedo = texture2D(gcolor, texcoord).rgb;
    vec2 extra = texture2D(gextra, texcoord).rg;
    float roughness = extra.r;
    float metallic = extra.g;

    vec3 N = normalize(texture2D(gnormal, texcoord).xyz);
    vec3 V = normalize(camera_pos - world_pos);
    
    vec3 F0 = vec3(0.04);
    F0 = mix(F0, albedo, metallic);
    
    vec3 radiance_out = vec3(0.0);
    for (int i = 0; i < NUM_LIGHTS; ++i) {
        vec3 L = normalize(light_positions[i].xyz - world_pos);
        vec3 H = normalize(V + L);
        float light_distance = length(light_positions[i].xyz - world_pos);
        float attenuation = 1.0 / pow(light_distance, 2);
        vec3 radiance_in = 100.0 * light_colors[i].rgb * attenuation; //+ vec3(0.3373, 0.5529, 0.8314);

        // Calculate the Cook-Torrance specular terms
        vec3 fresnel = brdf_fresnel_term(max(dot(N, H), 0.0), F0);
        float distribution = brdf_distribution_term(N, H, roughness);
        float geometry = brdf_geometry_term(N, V, L, roughness);

        vec3 numerator = distribution * geometry * fresnel;
        float denominator = 4 * max(dot(N, L), 0.0) * max(dot(N, V), 0.0);
        vec3 specular_term = numerator / max(denominator, 0.001);

        vec3 kS = fresnel;
        // multiply by `1 - metallic` because light that's not reflected is
        // absorbed my the free electrons in the metallic medium, so no
        // diffusion occurs.
        vec3 kD = (1 - kS) * (1 - metallic);

        vec3 brdf_term = kD * albedo / PI + specular_term;
        radiance_out += brdf_term * radiance_in * max(dot(N, L), 0.0);
    }

    // const vec3 SUN_DIR = normalize(vec3(1.0, 1.0, 0.3));

    // radiance_out += vec3(0.0941, 0.0745, 0.0392) * max(dot(reflect(V, N), SUN_DIR), 0.0);
    radiance_out += texture2D(gemissive, texcoord).rgb;

    vec3 ambient = vec3(pow(texture2D(ao, texcoord).r, 4));
    vec3 col = ambient * radiance_out;
    col = col / (col + 1.0);

    color = vec4(col, 1.0);
    // if (length(col) == 0) {
    //     color = vec4(1.0, 0.0, 1.0, 1.0);
    // }


    // // color = vec4(0.1, 0.1, 0.3, 0.0);
    // float dist = brdf_distribution_term(N, vec3(0.0, 1.0, 0.0), texture2D(gextra, texcoord).r);

    // if (texcoord.x > 0.5 && texcoord.y < 0.5) {
    //     vec3 snormal = texture2D(gnormal, vec2(1.0, 0.0) + texcoord * 2.0).xyz;
    //     // vec4 norm_view = view_matrix * vec4(snormal, 1.0);
    //     color = vec4(snormal, 1.0);
    // }
    // else if (texcoord.x < 0.5 && texcoord.y < 0.5) {
    //     vec2 tex = texcoord * 2.0;

    //     vec3 N = normalize(texture2D(gnormal, tex).xyz);
    //     vec3 V = normalize(camera_pos - texture2D(gposition, tex).xyz);
    //     vec3 L = vec3(0.0, 1.0, 0.0);
    //     float geometry = brdf_geometry_term(N, V, L);
    //     color = vec4(vec3(geometry), 1.0);
    // }
    // else if (texcoord.x < 0.5 && texcoord.y > 0.5) {
    //     vec2 tex = vec2(0.0, 1.0) + texcoord * 2.0;

    //     float roughness = texture2D(gextra, tex).r;
    //     vec3 N = normalize(texture2D(gnormal, tex).xyz);
    //     vec3 V = normalize(camera_pos - texture2D(gposition, tex).xyz);
    //     vec3 fresnel = brdf_fresnel_term(dot(N, V), vec3(0.04), roughness);
    //     // vec4 pos_view = view_matrix * vec4(spos, 1.0);
    //     color = vec4(fresnel, 1.0);
    // }
    // else if (texcoord.x > 0.5 && texcoord.y > 0.5) {
    //     // vec3 sao = texture2D(ao, 1.0 + texcoord * 2.0).rrr;
    //     vec2 tex = 1.0 + texcoord * 2.0;

    //     float roughness = texture2D(gextra, tex).r;
    //     vec3 N = normalize(texture2D(gnormal, tex).xyz);
    //     float dist = brdf_distribution_term(N, vec3(0.0, 1.0, 0.0), 0.1);
    //     color = vec4(vec3(dist), 1.0);
    // }

    // vec3 sao = texture2D(ao, texcoord).rrr;
    // vec3 salbedo = texture2D(gcolor, texcoord).rgb;

    // color = vec4(sao * salbedo, 1.0); //vec4(texture2D(gcolor, texcoord).rgb, 1.0);
}
