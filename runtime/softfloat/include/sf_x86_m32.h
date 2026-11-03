/* The x86 m32 configuration: a widening 32x32 multiply and Newton division (eight products per divide, against 28 restoring steps). */
#ifndef SF_X86_M32_H
#define SF_X86_M32_H
#define SF_MUL32X32(a, b, h, l) do { uint64_t p_ = (uint64_t)(a) * (b); *(h) = (uint32_t)(p_ >> 32); *(l) = (uint32_t)p_; } while (0)
#define SF_DIV_NEWTON 1
#endif
