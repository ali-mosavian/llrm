/* Vectors narrower than a register: `<8 x i8>`, `<4 x i16>`, `<2 x i32>` are widened to the register and touch
   only their own bytes (`movq`, never a 16-byte access). */
#define V(T, N) T __attribute__((vector_size(N * sizeof(T)), aligned(1)))
typedef V(unsigned char, 8) u8x8;
typedef V(unsigned short, 4) u16x4;
typedef V(unsigned, 2) u32x2;

void add_u8x8(u8x8 *o, const u8x8 *a, const u8x8 *b) { *o = *a + *b; }
void sub_u16x4(u16x4 *o, const u16x4 *a, const u16x4 *b) { *o = *a - *b; }
void xor_u32x2(u32x2 *o, const u32x2 *a, const u32x2 *b) { *o = *a ^ *b; }
void set_u8x8(u8x8 *o) { *o = (u8x8){1, 2, 3, 4, 5, 6, 7, 8}; }

/* A ramp of 48 bytes: the vector loop is fully unrolled and every vector it stores is a constant. */
static unsigned char ramp[48];
void fill_ramp(void) { for (int i = 0; i < 48; i++) ramp[i] = (unsigned char)(i * 5 - 20); }
unsigned char ramp_at(int i) { return ramp[i]; }
