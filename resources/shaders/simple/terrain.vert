#version 330 core

in uint pos_ao;
in uint side_id;

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
    vec3 pos = vec3(float(bx), float(by), float(bz));

    uint bid = side_id & uint(65535);
    int id = int(bid);
    
    uint baxis = (side_id >> 16) & uint(3);
    uint baxisSign = (side_id >> 18) & uint(1);

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

    float brightness = 1.0 - ((1.0 - ao) * (1.0 - MIN_AO_BRIGHTNESS));

    // directional lighting
    brightness *= contribution(baxis == uint(1) && baxisSign == uint(0), 0.0); // top
    brightness *= contribution(baxis == uint(1) && baxisSign == uint(1), 0.4); // bottom
    brightness *= contribution(baxis == uint(0), 0.1); // X
    brightness *= contribution(baxis == uint(2), 0.2); // Z

    v_color_filter = vec3(brightness);

    v_id = id;
    v_texture_uv = uv;
    v_normal = normal;
}
