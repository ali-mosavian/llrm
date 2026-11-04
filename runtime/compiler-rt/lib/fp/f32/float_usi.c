/* __floatunsisf; its own file so a program that needs only one of the two links only that one. */
#include "float_si.h"

sf_float SF_NAME(floatunsisf)(uint32_t x) { return b2f(from_mag(0, x)); }
