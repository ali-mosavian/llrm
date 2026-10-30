// RUN: llrm-cc %s -m32 -mabi=sysv -Os -march=i486 --isel=dag -S -o /dev/stdout
// At -Os m32 the select that sizes the memset was scheduled inside the call sequence, so the frame was not closed in its block and every argument
// went to [esp+k] (QCport weapons _pl_fire: 4 pushes where the pin has 98). The pin's source-order queue computes it first and pushes.
extern void *memset(void *, int, unsigned);
extern int use(char *, int);
int f(char *p, unsigned short n, char *q)
{
    char buf[100];
    memset(buf, 0, 2 * (n - 1 < 47 ? n - 1 : 47) + 2);
    return use(buf, q[0]);
}
// CHECK-LABEL: _f proc
// CHECK: pushd 0
// CHECK: call _memset
// CHECK: _f endp
