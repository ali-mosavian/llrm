// RUN: llrm-cc %s {-O2 | -Os} -march=i386 -S -o /dev/stdout
// A counter that ends at 0 is tested by the flags of its own step (`add`/`dec` then `jne`): no `test` or `cmp` in the loop.
// CHECK-LABEL: {{_up(@3)?}} proc
// CHECK: add {{[a-d]x|si|di}}, 4
// CHECK-NEXT: jne
// CHECK-NOT: cmp
// CHECK-NOT: test
// CHECK: {{_up(@3)?}} endp
// CHECK-LABEL: {{_down(@3)?}} proc
// CHECK: add {{[a-d]x|si|di}}, -2
// CHECK-NEXT: jne
// CHECK-NOT: cmp
// CHECK-NOT: test
// CHECK: {{_down(@3)?}} endp
int up(char *a)
{
    int t = 0, i;

    for (i = -100; i != 0; i += 4)
        t += a[i + 100];
    return t;
}

int down(int *a, int n)
{
    int t = 0, i = n;

    do {
        i -= 2;
        t += a[i];
    } while (i != 0);
    return t;
}
