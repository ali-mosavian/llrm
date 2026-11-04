/* The x86 m64 configuration: the m32 knobs (sf32 -Os 2285 -> 1756 B, -O2 2795 -> 2275 B with llrm-cc) and a native 64x64 -> 128 multiply (sf64 -Os 5328 -> 4785 B, -O2 6388 -> 5836 B). */
#ifndef SF_X86_M64_H
#define SF_X86_M64_H
#include "../x86_m32/config.h"
#define SF_MUL64X64(a, b, h, l) do { unsigned __int128 p_ = (unsigned __int128)(a) * (b); *(h) = (uint64_t)(p_ >> 64); *(l) = (uint64_t)p_; } while (0)
#endif
