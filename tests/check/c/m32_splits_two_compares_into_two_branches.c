// RUN: llrm-cc %s --target=x86_32-wat -mabi=sysv -march=i486 -O1 -S -o /dev/stdout
// sieve m32 -O1: the loop is entered only if `i <= 31 && i * i < n`. The pin's cost rule keeps the two compares
// one condition (`setb; setb; and; cmp 1; jne`, byte registers and 9 B for the join); at the 486/P5 rates
// two branches are smaller and faster (the second compare runs only if the first holds).
// CHECK-LABEL: _f proc
// CHECK-NOT: setb
// CHECK-NOT: cmp {{[a-d][lh]}}, 1
// CHECK: _f endp
void f(unsigned char *a, unsigned short n, unsigned short i)
{
    unsigned short m;
    if (i <= 31)
        for (m = (unsigned short)(i * i); m < n; m = (unsigned short)(m + i)) a[m] = 1;
}
