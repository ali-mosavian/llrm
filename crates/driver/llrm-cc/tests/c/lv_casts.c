/* Vector extends and truncates that span more than one register: `punpck`, `psra`, `pand` + `packuswb`. */
#define V(T, N) T __attribute__((vector_size(N * sizeof(T)), aligned(1)))
typedef V(unsigned char, 16) u8x16;
typedef V(signed char, 16) s8x16;
typedef V(unsigned short, 8) u16x8;
typedef V(unsigned short, 16) u16x16;
typedef V(short, 8) s16x8;
typedef V(short, 16) s16x16;
typedef V(unsigned, 4) u32x4;
typedef V(unsigned, 8) u32x8;
typedef V(unsigned, 16) u32x16;
typedef V(int, 8) s32x8;
typedef V(int, 16) s32x16;
typedef V(unsigned long, 4) u64x4;
typedef V(long, 4) s64x4;

void z8_16(u16x16 *o, const u8x16 *p) { *o = __builtin_convertvector(*p, u16x16); }
void s8_16(s16x16 *o, const s8x16 *p) { *o = __builtin_convertvector(*p, s16x16); }
void z8_32(u32x16 *o, const u8x16 *p) { *o = __builtin_convertvector(*p, u32x16); }
void s8_32(s32x16 *o, const s8x16 *p) { *o = __builtin_convertvector(*p, s32x16); }
void z16_32(u32x8 *o, const u16x8 *p) { *o = __builtin_convertvector(*p, u32x8); }
void s16_32(s32x8 *o, const s16x8 *p) { *o = __builtin_convertvector(*p, s32x8); }
void z32_64(u64x4 *o, const u32x4 *p) { *o = __builtin_convertvector(*p, u64x4); }
void s32_64(s64x4 *o, const V(int, 4) *p) { *o = __builtin_convertvector(*p, s64x4); }
void t16_8(u8x16 *o, const u16x16 *p) { *o = __builtin_convertvector(*p, u8x16); }
void t32_8(u8x16 *o, const u32x16 *p) { *o = __builtin_convertvector(*p, u8x16); }
void t32_16(u16x8 *o, const u32x8 *p) { *o = __builtin_convertvector(*p, u16x8); }
void t64_32(u32x4 *o, const u64x4 *p) { *o = __builtin_convertvector(*p, u32x4); }
