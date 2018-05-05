#version 330 core

#define MAX_LIGHTS 4

uniform vec3 u_CameraPosition;
uniform vec3 u_LightAmbient;

uniform vec3 u_Light[MAX_LIGHTS];
uniform vec3 u_LightColor[MAX_LIGHTS];
uniform float u_LightAttenuation[MAX_LIGHTS];
uniform sampler2D u_TextureMap;

in vec3 v_Pos;
in vec3 v_Normal;
in float v_FaceScalar;
in vec2 v_Uv;

out vec4 color;

void main()
{
    vec3 to_camera = normalize(u_CameraPosition - v_Pos);
    vec3 total = vec3(0.0);
    for (int i = 0; i < MAX_LIGHTS; i++)
    {
        // From surface to light
        vec3 to_light = u_Light[i] - v_Pos;
        // From light to surface
        vec3 incidence = -normalize(to_light);
        vec3 reflected = reflect(incidence, v_Normal);
        float specular_coefficient = max(pow(dot(to_camera, reflected), 30.0), 0.0);
        float diffuse_coefficient = max(dot(normalize(to_light), v_Normal), 0.0);
        float attenuation = 1.0/(1.0 + u_LightAttenuation[i] * pow(length(to_light), 2.0));
        vec3 diffuse = diffuse_coefficient * u_LightColor[i];
        vec3 specular = specular_coefficient * u_LightColor[i];

        total += attenuation * (diffuse + specular);
    }

    float density = 0.007;
    float gradient = 2.3;
    float fog = exp(-pow(length(u_CameraPosition - v_Pos) * density, gradient));
    vec4 tex_color = texture(u_TextureMap, v_Uv);
    vec4 col = v_FaceScalar * tex_color * vec4(u_LightAmbient + total, 1.0);

    color = mix(vec4(0.529411765, 0.807843137, 0.921568627, 1.0), col, fog);
}
