typedef signed char v16i8 __attribute__((vector_size(16)));
typedef unsigned char u16i8 __attribute__((vector_size(16)));
typedef short v8i16 __attribute__((vector_size(16)));
typedef unsigned short u8i16 __attribute__((vector_size(16)));
typedef int v4i32 __attribute__((vector_size(16)));
typedef unsigned u4i32 __attribute__((vector_size(16)));
typedef long long v2i64 __attribute__((vector_size(16)));
typedef unsigned long long u2i64 __attribute__((vector_size(16)));

void words_by_vector(u8i16 *r, u8i16 *a, u8i16 *b) { *r = *a >> *b; }
void dwords_by_vector(u4i32 *r, u4i32 *a, u4i32 *b) { *r = *a >> *b; }
void bytes_by_vector(u16i8 *r, u16i8 *a, u16i8 *b) { *r = *a >> *b; }
void signed_bytes_by_vector(v16i8 *r, v16i8 *a, v16i8 *b) { *r = *a >> *b; }
void quads_by_vector(u2i64 *r, u2i64 *a, u2i64 *b) { *r = *a >> *b; }
void signed_quads_by_vector(v2i64 *r, v2i64 *a, v2i64 *b) { *r = *a >> *b; }
void bytes_by_constant(v16i8 *r, v16i8 *a) { *r = *a << 3; }
void signed_bytes_by_constant(v16i8 *r, v16i8 *a) { *r = *a >> 3; }
void words_by_constants(v8i16 *r, v8i16 *a) { *r = *a << (v8i16){1, 2, 3, 4, 5, 6, 7, 8}; }
void unsigned_words_by_constants(u8i16 *r, u8i16 *a) { *r = *a >> (u8i16){1, 2, 3, 4, 5, 6, 7, 8}; }
void dwords_by_two_constants(u4i32 *r, u4i32 *a) { *r = *a >> (u4i32){1, 1, 3, 3}; }
void words_by_splat(v8i16 *r, v8i16 *a, int n) {
    short m = n;
    v8i16 s = {m, m, m, m, m, m, m, m};
    *r = *a << s;
}
void bytes_by_splat(v16i8 *r, v16i8 *a, int n) {
    signed char m = n;
    v16i8 s = {m, m, m, m, m, m, m, m, m, m, m, m, m, m, m, m};
    *r = *a << s;
}
void dwords_by_undef_splat(u4i32 *r, u4i32 *a) {
    u4i32 s;
    s[0] = 3;
    s[2] = 3;
    s[3] = 3;
    *r = *a >> s;
}
unsigned shift_loop(int n) {
    unsigned s = 0;
    for (int i = n; i >= 0; i--) s += (((unsigned)i * 3u) ^ ((unsigned)i >> 2));
    return s;
}
