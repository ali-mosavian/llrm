#include <stdio.h>
struct P { long a, b; };
struct Q { long a, b, c; };
__attribute__((noinline)) struct P mk(long x, long y) { struct P p = {x + 1, y * 2}; return p; }
__attribute__((noinline)) long sum(struct P p) { return p.a + p.b; }
__attribute__((noinline)) struct Q mq(long x) { struct Q q = {x, x + 1, x + 2}; return q; }
__attribute__((noinline)) long sq(struct Q q) { return q.a * q.b + q.c; }
int main(void) {
  struct P p = mk(3, 4); struct Q q = mq(5);
  printf("%ld %ld %ld %ld\n", p.a, p.b, sum(p), sq(q));
  return 0;
}
