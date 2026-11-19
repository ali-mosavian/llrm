// RUN: llrm-cc %s -m32 -Os -march=i486 --isel=dag -S -o /dev/stdout
// Twelve zero bytes are a double and a dword. The dword needs the integer zero
// register anyway, and the mixed form moved x_jpeg_single -Os +5 B through the
// allocation it changed: a fill is doubles only when it is whole doubles.
extern void use(char *);
void kernel(void)
{
    char a[12] = {0};
    use(a);
}
// CHECK-LABEL: kernel_ proc
// CHECK: mov dword ptr [esp
// CHECK-NOT: fldz
