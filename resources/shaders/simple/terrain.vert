#version 330 core

in uint pos_ao;
in uint uv_id;

uniform mat4 model;
uniform mat4 view;
uniform mat4 projection;

out vec3 v_color_filter;
out vec2 v_texture_uv;
flat out int v_id;

void main()
{
    // unpack attributes
    uint bao = pos_ao & uint(3);
    float ao = float(bao) / 3.0;

    uint bz = (pos_ao >> 2) & uint(2047);
    uint by = (pos_ao >> 12) & uint(2047);
    uint bx = (pos_ao >> 22) & uint(2047);
    vec3 pos = vec3(float(bx), float(by), float(bz));

    uint bid = uv_id & uint(65535);
    int id = int(bid);
    
    uint bv = (uv_id >> 21) & uint(63);
    uint bu = (uv_id >> 27) & uint(63);
    vec2 uv = vec2(float(bu), float(bv));

    // normal shader code
    gl_Position = projection * view * model * vec4(pos, 1.0);

    float ao_strength = (1.0 - ao) * 0.9;

    v_id = id;
    v_color_filter = vec3(1.0 - ao_strength);
    v_texture_uv = uv;
}
