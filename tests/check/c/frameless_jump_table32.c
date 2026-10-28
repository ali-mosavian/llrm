// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -O2 --isel=dag -S -o /dev/stdout
// A jump table is a jump to labels the table names, all at the depth of the jump: the frame-free walk refused it ("an indirect jump")
// and x_funcptr kept `push ebp; mov ebp, esp` and spilled its sum around the table (56 instructions, the pin 38).
// CHECK-LABEL: _table_sum proc
// CHECK-NOT: ebp
// CHECK: jmp dword ptr {{.*}}*4]
// CHECK-NOT: ebp
// CHECK: _table_sum endp
static int f0(int x) { return x + 3; }
static int f1(int x) { return x * 5; }
static int f2(int x) { return x ^ 0x55; }
static int f3(int x) { return x >> 1; }
static int (*table[4])(int) = { f0, f1, f2, f3 };
long table_sum(int n)
{
    int i, x = 1; long sum = 0;
    for (i = 0; i < n; ++i) { x = table[(i * 7 + (x & 3)) & 3](x) & 0xFFFF; sum += x; }
    return sum;
}
