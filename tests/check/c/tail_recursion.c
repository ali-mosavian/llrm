// RUN: llrm-c %s {-m16 | -m32} {-O2 | -Os} -march=i486 -fno-inline-functions -S -o /dev/stdout
// A call a function ends with was a call per level (bench/bintree 102097 instructions, gcc 44110;
// bench/hanoi 286704, gcc 143356); each is a loop, an add or a product on the way out an accumulator.
// CHECK-LABEL: {{_gcd(@3)?}} proc
// CHECK-NOT: call {{.*}}_gcd
// CHECK: {{_gcd(@3)?}} endp
// CHECK-LABEL: {{_sum(@3)?}} proc
// CHECK-NOT: call {{.*}}_sum
// CHECK: {{_sum(@3)?}} endp
// CHECK-LABEL: {{_fact(@3)?}} proc
// CHECK-NOT: call {{.*}}_fact
// CHECK: {{_fact(@3)?}} endp
// CHECK-LABEL: {{_moves(@3)?}} proc
// CHECK: call {{.*}}_moves
// CHECK: {{_moves(@3)?}} endp
// CHECK-LABEL: {{_insert(@3)?}} proc
// CHECK-NOT: call {{.*}}_insert
// CHECK: {{_insert(@3)?}} endp
// CHECK-LABEL: {{_sub(@3)?}} proc
// CHECK: call {{.*}}_sub
// CHECK: {{_sub(@3)?}} endp
// CHECK-LABEL: {{_exposed(@3)?}} proc
// CHECK: call {{.*}}_exposed
// CHECK: {{_exposed(@3)?}} endp
int __cdecl gcd(int a, int b) { return b == 0 ? a : gcd(b, a % b); }
int __cdecl sum(int n) { return n == 0 ? 0 : n + sum(n - 1); }
int __cdecl fact(int n) { return n <= 1 ? 1 : n * fact(n - 1); }
int __cdecl moves(int n) { return n == 0 ? 0 : moves(n - 1) + 1 + moves(n - 1); }
int tree[300];
void __cdecl insert(int at, int v) { if (tree[at] == 0) tree[at] = v; else insert(at + 1, v); }
int __cdecl sub(int n) { return n == 0 ? 0 : n - sub(n - 1); }
int __cdecl exposed(int n, int *p) { int cell = n; return n == 0 ? 0 : exposed(n - 1, &cell) + *p; }
