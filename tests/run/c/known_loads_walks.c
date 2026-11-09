// flags: -O0 | -O2 | -Os
// One function of ~250 blocks (nested loops and branches on bytes, zeros assigned throughout): the values the peephole pass tracks at its labels need more than eight walks to settle, so it left every redundant `lda #0` in. The program runs on every target; expected output from the same source built for the host.
extern void report(long value);

__attribute__((noinline)) unsigned char f(unsigned char a, unsigned char b, unsigned char m)
{
    unsigned char c = 0, d = 0, e = 0;
    a = c;
    for (unsigned char i1 = 0; i1 < (unsigned char)(m + 0); ++i1) {
    c = 0;
    for (unsigned char i2 = 0; i2 < (unsigned char)(m + 0); ++i2) {
    e = a;
    for (unsigned char i3 = 0; i3 < (unsigned char)(m + 1); ++i3) {
    a = (unsigned char)(c + c);
    c = a;
    c = 0;
    b = (unsigned char)(c | 7);
    }
    e = e;
    a = b;
    }
    c = a;
    for (unsigned char i4 = 0; i4 < (unsigned char)(m + 0); ++i4) {
    c = 0;
    if (b > e) {
    e = 255;
    } else {
    for (unsigned char i5 = 0; i5 < (unsigned char)(m + 1); ++i5) {
    e = c;
    d = 5;
    a = 2;
    d = 1;
    }
    }
    }
    }
    for (unsigned char i6 = 0; i6 < (unsigned char)(m + 0); ++i6) {
    d = d;
    e = c;
    d = (unsigned char)(b & d);
    if (b < a) {
    for (unsigned char i7 = 0; i7 < (unsigned char)(m + 1); ++i7) {
    for (unsigned char i8 = 0; i8 < (unsigned char)(m + 1); ++i8) {
    c = (unsigned char)(d | c);
    d = d;
    d = 5;
    }
    for (unsigned char i9 = 0; i9 < (unsigned char)(m + 1); ++i9) {
    c = d;
    b = (unsigned char)(c + e);
    a = 2;
    a = c;
    }
    }
    if (d < b) {
    e = 255;
    } else {
    if (d > a) {
    b = b;
    b = 5;
    } else {
    a = d;
    }
    a = e;
    for (unsigned char i10 = 0; i10 < (unsigned char)(m + 0); ++i10) {
    c = (unsigned char)(d | 3);
    b = (unsigned char)(b | d);
    }
    }
    d = d;
    } else {
    c = (unsigned char)(c & a);
    for (unsigned char i11 = 0; i11 < (unsigned char)(m + 1); ++i11) {
    for (unsigned char i12 = 0; i12 < (unsigned char)(m + 1); ++i12) {
    c = 0;
    e = e;
    b = 0;
    }
    for (unsigned char i13 = 0; i13 < (unsigned char)(m + 0); ++i13) {
    a = 5;
    e = 5;
    a = 0;
    a = 5;
    }
    for (unsigned char i14 = 0; i14 < (unsigned char)(m + 1); ++i14) {
    e = (unsigned char)(b ^ 3);
    b = b;
    b = a;
    e = c;
    }
    }
    e = d;
    }
    }
    e = b;
    e = (unsigned char)(c & 3);
    for (unsigned char i15 = 0; i15 < (unsigned char)(m + 0); ++i15) {
    b = a;
    for (unsigned char i16 = 0; i16 < (unsigned char)(m + 1); ++i16) {
    for (unsigned char i17 = 0; i17 < (unsigned char)(m + 0); ++i17) {
    if (a < e) {
    b = (unsigned char)(c + d);
    } else {
    a = 5;
    }
    if (a > a) {
    d = c;
    b = 255;
    e = (unsigned char)(b & e);
    } else {
    c = (unsigned char)(a + c);
    }
    if (a > 100) {
    c = a;
    } else {
    a = 1;
    c = 2;
    }
    b = 1;
    }
    if (b != e) {
    d = (unsigned char)(c ^ c);
    b = (unsigned char)(a ^ e);
    } else {
    a = a;
    }
    }
    }
    for (unsigned char i18 = 0; i18 < (unsigned char)(m + 1); ++i18) {
    if (e != b) {
    b = (unsigned char)(d + c);
    if (c > c) {
    c = 255;
    if (a != e) {
    a = 1;
    b = a;
    d = (unsigned char)(c | 7);
    } else {
    a = 0;
    e = b;
    }
    } else {
    if (a < 100) {
    b = b;
    b = 0;
    b = (unsigned char)(d + 3);
    } else {
    b = b;
    c = (unsigned char)(c + a);
    a = d;
    }
    if (a == 3) {
    e = 5;
    } else {
    b = d;
    a = (unsigned char)(b + d);
    }
    if (b != c) {
    d = (unsigned char)(e ^ d);
    } else {
    e = (unsigned char)(c & d);
    a = b;
    }
    }
    e = (unsigned char)(e | e);
    } else {
    for (unsigned char i19 = 0; i19 < (unsigned char)(m + 0); ++i19) {
    for (unsigned char i20 = 0; i20 < (unsigned char)(m + 0); ++i20) {
    b = 0;
    c = 5;
    a = 255;
    b = 1;
    }
    b = e;
    a = a;
    }
    a = e;
    for (unsigned char i21 = 0; i21 < (unsigned char)(m + 1); ++i21) {
    for (unsigned char i22 = 0; i22 < (unsigned char)(m + 0); ++i22) {
    e = (unsigned char)(c & 3);
    c = (unsigned char)(c + 7);
    b = (unsigned char)(a & 7);
    }
    e = 5;
    for (unsigned char i23 = 0; i23 < (unsigned char)(m + 1); ++i23) {
    b = c;
    c = (unsigned char)(c | d);
    a = (unsigned char)(c | d);
    }
    }
    }
    c = 0;
    e = d;
    }
    for (unsigned char i24 = 0; i24 < (unsigned char)(m + 1); ++i24) {
    a = 255;
    e = 1;
    for (unsigned char i25 = 0; i25 < (unsigned char)(m + 0); ++i25) {
    b = (unsigned char)(b + 7);
    for (unsigned char i26 = 0; i26 < (unsigned char)(m + 0); ++i26) {
    if (d != e) {
    c = (unsigned char)(d ^ 7);
    a = 255;
    b = d;
    } else {
    c = e;
    e = (unsigned char)(a | b);
    e = (unsigned char)(a & c);
    }
    c = (unsigned char)(d | c);
    if (e == e) {
    e = 0;
    } else {
    b = 2;
    e = d;
    a = d;
    }
    for (unsigned char i27 = 0; i27 < (unsigned char)(m + 0); ++i27) {
    d = a;
    e = c;
    b = a;
    e = 0;
    }
    }
    }
    b = e;
    }
    for (unsigned char i28 = 0; i28 < (unsigned char)(m + 1); ++i28) {
    if (a == d) {
    for (unsigned char i29 = 0; i29 < (unsigned char)(m + 1); ++i29) {
    for (unsigned char i30 = 0; i30 < (unsigned char)(m + 0); ++i30) {
    a = a;
    d = 1;
    }
    for (unsigned char i31 = 0; i31 < (unsigned char)(m + 1); ++i31) {
    a = (unsigned char)(c | b);
    c = 5;
    }
    }
    c = (unsigned char)(e + e);
    } else {
    for (unsigned char i32 = 0; i32 < (unsigned char)(m + 0); ++i32) {
    if (a < 3) {
    e = b;
    } else {
    a = e;
    c = (unsigned char)(b | b);
    a = 0;
    }
    if (c > e) {
    a = b;
    } else {
    d = e;
    e = (unsigned char)(a | 7);
    e = (unsigned char)(b ^ 3);
    }
    c = d;
    d = 0;
    }
    }
    for (unsigned char i33 = 0; i33 < (unsigned char)(m + 1); ++i33) {
    c = c;
    if (c != 100) {
    e = (unsigned char)(e & 3);
    for (unsigned char i34 = 0; i34 < (unsigned char)(m + 1); ++i34) {
    e = 0;
    c = a;
    e = 255;
    a = a;
    }
    } else {
    if (c < e) {
    b = 0;
    e = a;
    } else {
    a = 2;
    b = 0;
    c = (unsigned char)(e | d);
    }
    a = 5;
    }
    e = 0;
    }
    for (unsigned char i35 = 0; i35 < (unsigned char)(m + 1); ++i35) {
    c = 255;
    for (unsigned char i36 = 0; i36 < (unsigned char)(m + 0); ++i36) {
    e = a;
    for (unsigned char i37 = 0; i37 < (unsigned char)(m + 0); ++i37) {
    d = 255;
    e = c;
    d = e;
    }
    }
    }
    }
    if (b < 100) {
    for (unsigned char i38 = 0; i38 < (unsigned char)(m + 1); ++i38) {
    if (a != c) {
    b = 0;
    for (unsigned char i39 = 0; i39 < (unsigned char)(m + 0); ++i39) {
    b = (unsigned char)(a & 3);
    a = 0;
    }
    for (unsigned char i40 = 0; i40 < (unsigned char)(m + 1); ++i40) {
    e = 1;
    a = c;
    b = d;
    b = 5;
    }
    } else {
    for (unsigned char i41 = 0; i41 < (unsigned char)(m + 0); ++i41) {
    a = 2;
    c = (unsigned char)(a & b);
    c = (unsigned char)(e & 7);
    d = c;
    }
    if (c > a) {
    c = 0;
    a = 0;
    a = (unsigned char)(c + d);
    } else {
    a = (unsigned char)(c ^ 7);
    }
    b = 0;
    }
    a = c;
    d = 0;
    e = d;
    }
    e = (unsigned char)(c + e);
    c = (unsigned char)(d + 3);
    } else {
    for (unsigned char i42 = 0; i42 < (unsigned char)(m + 0); ++i42) {
    for (unsigned char i43 = 0; i43 < (unsigned char)(m + 0); ++i43) {
    for (unsigned char i44 = 0; i44 < (unsigned char)(m + 1); ++i44) {
    e = 0;
    c = (unsigned char)(a ^ c);
    d = a;
    }
    if (b < d) {
    b = 5;
    a = 1;
    } else {
    c = (unsigned char)(b ^ b);
    }
    if (e == b) {
    a = 0;
    } else {
    d = (unsigned char)(d & b);
    a = 0;
    c = (unsigned char)(d & 3);
    }
    if (b != b) {
    b = b;
    } else {
    d = (unsigned char)(b | 3);
    }
    }
    if (d != 3) {
    a = (unsigned char)(a & a);
    if (e != d) {
    d = 255;
    } else {
    a = 255;
    }
    for (unsigned char i45 = 0; i45 < (unsigned char)(m + 0); ++i45) {
    b = b;
    a = e;
    }
    } else {
    c = 0;
    a = (unsigned char)(b ^ 3);
    for (unsigned char i46 = 0; i46 < (unsigned char)(m + 1); ++i46) {
    b = c;
    b = c;
    a = (unsigned char)(a + c);
    }
    }
    }
    for (unsigned char i47 = 0; i47 < (unsigned char)(m + 1); ++i47) {
    d = c;
    b = 0;
    for (unsigned char i48 = 0; i48 < (unsigned char)(m + 0); ++i48) {
    c = e;
    if (d < 100) {
    c = c;
    c = 5;
    } else {
    c = (unsigned char)(a | 3);
    e = (unsigned char)(b & d);
    c = c;
    }
    }
    for (unsigned char i49 = 0; i49 < (unsigned char)(m + 1); ++i49) {
    for (unsigned char i50 = 0; i50 < (unsigned char)(m + 1); ++i50) {
    a = (unsigned char)(d ^ d);
    e = c;
    }
    for (unsigned char i51 = 0; i51 < (unsigned char)(m + 0); ++i51) {
    e = d;
    b = (unsigned char)(c & b);
    }
    d = 255;
    for (unsigned char i52 = 0; i52 < (unsigned char)(m + 0); ++i52) {
    b = 255;
    a = d;
    c = b;
    }
    }
    }
    }
    if (a > a) {
    a = d;
    for (unsigned char i53 = 0; i53 < (unsigned char)(m + 1); ++i53) {
    for (unsigned char i54 = 0; i54 < (unsigned char)(m + 0); ++i54) {
    d = 2;
    if (e < 100) {
    d = b;
    b = (unsigned char)(b & c);
    e = d;
    } else {
    e = 5;
    d = (unsigned char)(c ^ 7);
    d = (unsigned char)(a + 7);
    }
    }
    for (unsigned char i55 = 0; i55 < (unsigned char)(m + 0); ++i55) {
    if (d < 3) {
    c = b;
    } else {
    d = a;
    a = (unsigned char)(d & e);
    c = 0;
    }
    a = b;
    c = 0;
    if (b > d) {
    b = (unsigned char)(e + d);
    } else {
    b = b;
    d = 0;
    }
    }
    for (unsigned char i56 = 0; i56 < (unsigned char)(m + 0); ++i56) {
    for (unsigned char i57 = 0; i57 < (unsigned char)(m + 0); ++i57) {
    e = 0;
    e = (unsigned char)(e ^ e);
    c = (unsigned char)(e & a);
    }
    if (d > b) {
    c = 255;
    } else {
    b = b;
    }
    if (a != 3) {
    a = b;
    } else {
    e = c;
    a = d;
    c = d;
    }
    }
    e = e;
    }
    b = (unsigned char)(b & 3);
    } else {
    d = (unsigned char)(e | a);
    for (unsigned char i58 = 0; i58 < (unsigned char)(m + 0); ++i58) {
    b = c;
    c = 1;
    b = 0;
    }
    a = b;
    }
    a = 0;
    d = (unsigned char)(b + 3);
    a = e;
    for (unsigned char i59 = 0; i59 < (unsigned char)(m + 1); ++i59) {
    for (unsigned char i60 = 0; i60 < (unsigned char)(m + 1); ++i60) {
    for (unsigned char i61 = 0; i61 < (unsigned char)(m + 1); ++i61) {
    for (unsigned char i62 = 0; i62 < (unsigned char)(m + 0); ++i62) {
    e = (unsigned char)(d ^ b);
    d = 0;
    d = b;
    b = 1;
    }
    for (unsigned char i63 = 0; i63 < (unsigned char)(m + 0); ++i63) {
    a = (unsigned char)(e + b);
    d = 1;
    }
    if (b == e) {
    d = c;
    c = 0;
    } else {
    a = c;
    }
    }
    if (b != b) {
    for (unsigned char i64 = 0; i64 < (unsigned char)(m + 1); ++i64) {
    e = a;
    a = a;
    }
    } else {
    if (c < 3) {
    e = (unsigned char)(e + c);
    } else {
    e = 0;
    a = b;
    a = (unsigned char)(b ^ 3);
    }
    d = d;
    if (a == 3) {
    e = 1;
    } else {
    e = (unsigned char)(d & c);
    a = b;
    c = b;
    }
    }
    }
    if (a != d) {
    if (c < 100) {
    for (unsigned char i65 = 0; i65 < (unsigned char)(m + 0); ++i65) {
    d = (unsigned char)(a ^ e);
    c = e;
    d = b;
    }
    } else {
    d = 5;
    }
    if (b != e) {
    a = (unsigned char)(e + d);
    e = (unsigned char)(a ^ d);
    } else {
    c = 5;
    for (unsigned char i66 = 0; i66 < (unsigned char)(m + 0); ++i66) {
    a = 5;
    a = b;
    e = e;
    }
    }
    } else {
    for (unsigned char i67 = 0; i67 < (unsigned char)(m + 0); ++i67) {
    for (unsigned char i68 = 0; i68 < (unsigned char)(m + 1); ++i68) {
    b = (unsigned char)(a ^ 7);
    d = d;
    b = a;
    e = 5;
    }
    a = d;
    for (unsigned char i69 = 0; i69 < (unsigned char)(m + 0); ++i69) {
    d = (unsigned char)(a & 3);
    c = 255;
    }
    }
    if (d > 3) {
    if (d == e) {
    d = (unsigned char)(c & b);
    } else {
    e = 2;
    a = d;
    d = (unsigned char)(d & 7);
    }
    a = (unsigned char)(c & d);
    for (unsigned char i70 = 0; i70 < (unsigned char)(m + 0); ++i70) {
    c = 0;
    a = (unsigned char)(e ^ d);
    d = (unsigned char)(a & c);
    c = (unsigned char)(c + d);
    }
    } else {
    if (c == d) {
    c = 0;
    e = 2;
    } else {
    d = 0;
    a = (unsigned char)(c ^ 7);
    e = 0;
    }
    a = 1;
    }
    for (unsigned char i71 = 0; i71 < (unsigned char)(m + 0); ++i71) {
    e = 1;
    if (b == b) {
    d = 0;
    b = (unsigned char)(c & 7);
    } else {
    d = c;
    }
    e = (unsigned char)(c ^ 7);
    for (unsigned char i72 = 0; i72 < (unsigned char)(m + 0); ++i72) {
    c = 0;
    c = c;
    }
    }
    }
    for (unsigned char i73 = 0; i73 < (unsigned char)(m + 0); ++i73) {
    for (unsigned char i74 = 0; i74 < (unsigned char)(m + 0); ++i74) {
    for (unsigned char i75 = 0; i75 < (unsigned char)(m + 1); ++i75) {
    a = (unsigned char)(b ^ b);
    c = b;
    }
    d = 1;
    if (e > c) {
    d = 0;
    a = 5;
    b = 0;
    } else {
    b = 0;
    b = d;
    }
    }
    if (b == 100) {
    d = (unsigned char)(a + 3);
    } else {
    for (unsigned char i76 = 0; i76 < (unsigned char)(m + 0); ++i76) {
    a = c;
    c = 255;
    }
    d = 1;
    }
    if (e > b) {
    if (a < 3) {
    c = a;
    } else {
    c = 5;
    c = 0;
    e = (unsigned char)(c ^ d);
    }
    c = 0;
    } else {
    if (d != e) {
    e = (unsigned char)(c + 7);
    d = 0;
    } else {
    b = 0;
    }
    for (unsigned char i77 = 0; i77 < (unsigned char)(m + 0); ++i77) {
    c = b;
    c = 5;
    }
    a = e;
    }
    for (unsigned char i78 = 0; i78 < (unsigned char)(m + 0); ++i78) {
    for (unsigned char i79 = 0; i79 < (unsigned char)(m + 0); ++i79) {
    b = (unsigned char)(e | 3);
    e = a;
    e = (unsigned char)(d & 3);
    }
    c = c;
    c = 0;
    c = (unsigned char)(a & b);
    }
    }
    }
    b = 0;
    return (unsigned char)(a + 3 * b + 5 * c + 7 * d + 11 * e);
}

int main(void)
{
    static volatile unsigned char two = 2;
    unsigned long s = 0;
    unsigned char x, y;
    for (x = 0; x < 4; ++x)
        for (y = 0; y < 4; ++y)
            s = (s * 31 + f((unsigned char)(x * 37 + two), (unsigned char)(y * 11 + 5), two)) & 0xFFFFFFFFUL;
    report(s & 0xFFFF);
    report((s >> 16) & 0xFFFF);
    return 0;
}
