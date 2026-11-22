#include <stdio.h>
#include <string.h>
#include <math.h>
typedef float v4 __attribute__((vector_size(16)));
typedef double v2d __attribute__((vector_size(16)));
typedef int i4 __attribute__((vector_size(16)));
typedef long long i2 __attribute__((vector_size(16)));
#define OPS(X) X(lt,<) X(le,<=) X(gt,>) X(ge,>=) X(eq,==) X(ne,!=)
#define F(n,o) __attribute__((noinline)) void f_##n(v4 *a, v4 *b, i4 *r){ *r = *a o *b; } \
               __attribute__((noinline)) void d_##n(v2d *a, v2d *b, i2 *r){ *r = *a o *b; }
OPS(F)
__attribute__((noinline)) void sel(v4 *a, v4 *b, i4 *x, i4 *y, i4 *r){ i4 m = *a < *b; *r = (m & *x) | (~m & *y); }
__attribute__((noinline)) void isuno(v4 *a, v4 *b, i4 *r){ *r = (*a != *a) | (*b != *b); }
__attribute__((noinline)) void one(v4 *a, v4 *b, i4 *r){ *r = (*a < *b) | (*a > *b); }
__attribute__((noinline)) void ueq(v4 *a, v4 *b, i4 *r){ *r = ~((*a < *b) | (*a > *b)); }
static void pi(const char *n, i4 v){ int t[4]; memcpy(t,&v,16); printf("%s %d %d %d %d\n", n, t[0]&1, t[1]&1, t[2]&1, t[3]&1); }
static void pl(const char *n, i2 v){ long long t[2]; memcpy(t,&v,16); printf("%s %lld %lld\n", n, t[0]&1, t[1]&1); }
int main(void){
  volatile float n = NAN;
  v4 a = {1, 2, n, 3}, b = {2, 2, 1, n};
  v2d c = {1, n}, d = {2, 1}, e = {2, 2}; i4 r4; i2 r2;
#define P(nm,o) f_##nm(&a,&b,&r4); pi(#nm, r4); d_##nm(&c,&d,&r2); pl("d" #nm, r2); d_##nm(&c,&e,&r2); pl("e" #nm, r2);
  OPS(P)
  i4 x = {10,20,30,40}, y = {-1,-2,-3,-4}; i4 s; sel(&a,&b,&x,&y,&s);
  printf("sel %d %d %d %d\n", s[0], s[1], s[2], s[3]);
  isuno(&a,&b,&r4); pi("uno", r4); one(&a,&b,&r4); pi("one", r4); ueq(&a,&b,&r4); pi("ueq", r4);
  return 0;
}
