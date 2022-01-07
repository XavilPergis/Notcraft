#pragma shaderstage vertex
#version 330 core

in uint pos_ao;
in uint light_side_id;

uniform mat4 model;
uniform mat4 view;
uniform mat4 projection;

out vec3 v_color_filter;
out vec3 v_normal;
out vec2 v_texture_uv;
flat out int v_id;

float contribution(bool contribute, float strength) {
    return 1.0 - float(contribute) * strength;
}

#define MIN_AO_BRIGHTNESS (0.2)

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
    uint bao = pos_ao & uint(3);
    float ao = float(bao) / 3.0;

    uint bz = (pos_ao >> 2) & uint(1023);
    uint by = (pos_ao >> 12) & uint(1023);
    uint bx = (pos_ao >> 22) & uint(1023);
    vec3 pos = vec3(float(bx) / 16.0, float(by) / 16.0, float(bz) / 16.0);

    uint bid = light_side_id & uint(65535);
    int id = int(bid);
    
    uint light_side = light_side_id >> 16;
    uint baxis = light_side & uint(3);
    uint baxisSign = (light_side >> 2) & uint(1);

    uint light = (light_side >> 8) & uint(1);
    float blockLight = float(light & uint(15)) / 16.0;
    float skyLight = float((light >> 4) & uint(15)) / 16.0;

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
    brightness *= mix(MIN_AO_BRIGHTNESS, 1.0, ao);

    // directional lighting
    brightness *= contribution(baxis == uint(1) && baxisSign == uint(0), 0.0); // top
    brightness *= contribution(baxis == uint(1) && baxisSign == uint(1), 0.4); // bottom
    brightness *= contribution(baxis == uint(0), 0.1); // X
    brightness *= contribution(baxis == uint(2), 0.2); // Z

    brightness *= max(
        mix(0.1, 1.0, blockLight),
        mix(0.1, 1.0, skyLight)
    );

    v_color_filter = vec3(brightness);

    v_id = id;
    v_texture_uv = uv;
    v_normal = normal;
}

#pragma shaderstage fragment
#version 330 core

uniform sampler2DArray albedo_maps;

in vec3 v_color_filter;
in vec2 v_texture_uv;
flat in int v_id;

out vec3 b_color;

void main()
{
    vec3 tex_pos = vec3(v_texture_uv, v_id);
    vec4 tex = texture(albedo_maps, tex_pos);
    vec3 albedo = v_color_filter * tex.rgb;
    if (tex.a < 0.5) {
        discard;
    }
    b_color = albedo;
}
