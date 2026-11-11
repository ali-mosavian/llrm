// RUN: llrm-cc %s -m16 -O1 -S -o /dev/stdout
// At m16 -O1 (bytes strict, no speed rate) a long remainder by a constant
// stays an `idiv`, as at -Os. It was the reciprocal multiply, ~30 B in real
// mode (66 prefix per 32-bit op): recmany's three `% 5/3/7` made 394 B
// against bcc's 191.
long r(long a) { return a % 5; }
// CHECK-LABEL: _r@3 proc
// CHECK: idiv ecx
// CHECK-NOT: imul
// CHECK: _r@3 endp
