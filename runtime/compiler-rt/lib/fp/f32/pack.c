/* Round to nearest even and pack (see softfloat.h). */
#include "f32.h"

sf32 sf32_pack(uint32_t sign, int e, uint32_t m) { return pack_any(sign, e, m); }

#include "../core_end.inc"
