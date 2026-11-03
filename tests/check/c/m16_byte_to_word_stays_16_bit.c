// RUN: llrm-cc %s -m16 -march=i386 -O2 --isel=dag -S -o /dev/stdout
// grep c m16 -O2 widened each byte table lookup: `movzx esi, byte ptr _nxt[bx+si]` (a 66h prefix, and a
// 4-byte slot where the value spilled) for a value the program holds in 16 bits. The word form is `movzx si, ...`.
// CHECK-LABEL: walk
// CHECK: movzx si, byte ptr
// CHECK: movzx si, byte ptr
// CHECK: movzx cx, byte ptr
// CHECK-NOT: movzx e
unsigned char nxt[800], hit[80];
int walk(const unsigned char *t, int n) {
    int s = 0, lines = 0, i;
    for (i = 0; i < n; ++i) {
        s = nxt[s * 10 + t[i]];
        if (hit[s]) lines += hit[s];
    }
    return lines;
}
