#pragma shaderstage vertex
#version 330 core

#pragma include "/util.glsl"

#define AXIS_X 0
#define AXIS_Y 1
#define AXIS_Z 2
#define SIGN_POSITIVE 0
#define SIGN_NEGATIVE 1

in uint pos_ao;
in uint light_flags_side_id;

struct TerrainVertex {
    vec3 modelPos;
    vec3 modelNormal;
    int axis;
    int axisSign;

    int  textureId;
    vec2 textureCoordinates;

    float blockLight;
    float skyLight;
    float ao;
    bool  windSway;
};

TerrainVertex unpackVertex() {
    vec3 normalTable[3];
    float signTable[2];

    normalTable = vec3[3](
        vec3(1.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        vec3(0.0, 0.0, 1.0)
    );
    signTable = float[2](1.0, -1.0);

    // unpack attributes
    float ao = float(BITS(pos_ao, 0, 2)) / 3.0;
    float z  = float(BITS(pos_ao, 2, 10)) / 16.0;
    float y  = float(BITS(pos_ao, 12, 10)) / 16.0;
    float x  = float(BITS(pos_ao, 22, 10)) / 16.0;

    int textureId    = int  (BITS(light_flags_side_id, 0, 16));
    int axis         = int  (BITS(light_flags_side_id, 16, 2));
    int axisSign     = int  (BITS(light_flags_side_id, 18, 1));
    bool windSway    = bool (BITS(light_flags_side_id, 23, 1));
    float blockLight = float(BITS(light_flags_side_id, 24, 4)) / 16.0;
    float skyLight   = float(BITS(light_flags_side_id, 28, 4)) / 16.0;

    vec3 modelPos = vec3(x, y, z);
    vec3 modelNormal = normalTable[axis];
    modelNormal *= signTable[axisSign];

    vec2 uvTable[3];
    uvTable = vec2[3](
        vec2(z, y),
        vec2(x, z),
        vec2(x, y)
    );
    vec2 textureCoordinates = uvTable[axis];


    return TerrainVertex(
        modelPos,
        modelNormal,
        axis,
        axisSign,
        textureId,
        textureCoordinates,
        blockLight,
        skyLight,
        ao,
        windSway
    );
}

