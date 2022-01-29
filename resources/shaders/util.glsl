#define PI 3.1415926535
#define BITS(packed, a, n) ((packed) >> (a)) & uint((1 << (n)) - 1)
#define RGB(r, g, b) (vec3(r.0 / 255.0, g.0 / 255.0, b.0 / 255.0))
