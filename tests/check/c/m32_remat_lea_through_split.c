// RUN: llrm-cc %s --target=x86_32-wat -mabi=sysv -march=i486 -Os -S -o /dev/stdout
// InlineSpiller `reMaterializeFor`: a register-reading `lea` (`n * 4`) that a split carries is made again where it
// is read, from the instruction now at the original value's def, not stored and reloaded. x_transpose -Os m32 kept
// two spill cells (`sub esp, 8`, 194 B) where the pin keeps one (`push eax`, 188 B).
// CHECK-LABEL: _bench_x_transpose proc
// CHECK-NOT: sub esp, 8
// CHECK: _bench_x_transpose endp

static int M[32][32], T[32][32];
long bench_x_transpose(int n)
{
    int i, j, r; long sum = 0;
    for (i = 0; i < n; ++i) for (j = 0; j < n; ++j) M[i][j] = i * 37 + j;
    for (r = 0; r < 8; ++r) { for (i = 0; i < n; ++i) for (j = 0; j < n; ++j) T[j][i] = M[i][j] + r; for (i = 0; i < n; ++i) for (j = 0; j < n; ++j) M[i][j] = T[i][j]; }
    for (i = 0; i < n; ++i) sum += M[i][n - 1 - i];
    return sum;
}

