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
    
    vec3 normal = normalFromAxis[(side_id >> 16) & uint(3)];
    normal *= axisSign[(side_id >> 18) & uint(1)];

    vec2 uvFromAxis[3];
    uvFromAxis = vec2[3](
        vec2(pos.z, pos.y),
        vec2(pos.x, pos.z),
        vec2(pos.x, pos.y)
    );
    vec2 uv = uvFromAxis[(side_id >> 16) & uint(3)];

    // normal shader code
    gl_Position = projection * view * model * vec4(pos, 1.0);

    float ao_strength = (1.0 - ao) * 0.9;
    v_color_filter = vec3(1.0 - ao_strength);

    v_id = id;
    v_texture_uv = uv;
    v_normal = normal;
}
