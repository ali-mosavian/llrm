/* The single-precision math functions clang calls for its float intrinsics; the DOS C libraries (Borland, Open Watcom) have the double ones only. */
#include <math.h>

float fabsf(float x) { return (float)fabs(x); }
float sqrtf(float x) { return (float)sqrt(x); }
float sinf(float x) { return (float)sin(x); }
float cosf(float x) { return (float)cos(x); }
float tanf(float x) { return (float)tan(x); }
float atanf(float x) { return (float)atan(x); }
float atan2f(float y, float x) { return (float)atan2(y, x); }
float expf(float x) { return (float)exp(x); }
float logf(float x) { return (float)log(x); }
float powf(float x, float y) { return (float)pow(x, y); }
float floorf(float x) { return (float)floor(x); }
float ceilf(float x) { return (float)ceil(x); }
float fmodf(float x, float y) { return (float)fmod(x, y); }
