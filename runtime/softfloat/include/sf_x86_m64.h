/* The x86 m64 configuration: a native 64x64 -> 128 multiply (sf64 -Os 5328 -> 4785 B, -O2 6388 -> 5836 B with llrm-cc). */
#ifndef SF_X86_M64_H
#define SF_X86_M64_H
#define SF_MUL64X64(a, b, h, l) do { unsigned __int128 p_ = (unsigned __int128)(a) * (b); *(h) = (uint64_t)(p_ >> 64); *(l) = (uint64_t)p_; } while (0)
#endif
