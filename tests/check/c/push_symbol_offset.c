// RUN: llrm-cc %s -m32 -mabi=sysv -Os -march=i486 --isel=dag -S -o /dev/stdout
// An extern symbol plus a constant was `lea eax, _tab+12; push eax`. The pin's static model folds the offset into the symbol: `push offset tab+12`.
extern int tab[10];
extern void use(int *, int);
void f(int x) { use(tab + 3, x); }
// CHECK-LABEL: _f proc
// CHECK: push {{.*}}_tab+12
// CHECK-NOT: lea
