// RUN: llrm-cc %s -m16 -Os --isel=dag -S -o /dev/stdout
// An 18-byte zeroed local was five dword stores (~24 B over `rep stosb`)
// at -Os; the old selector filled it.
int sink(int *);
int zeroed(void)
{
    int a[9] = {0};
    return sink(a);
}
// CHECK-LABEL: _zeroed@3 proc
// CHECK: rep stos
