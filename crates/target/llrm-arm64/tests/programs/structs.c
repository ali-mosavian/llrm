int printf(const char *, ...);
struct point { int x, y; };
struct big { long a; char b; short c; double d; struct point p; };
static struct point add(struct point a, struct point b) { struct point r = {a.x + b.x, a.y + b.y}; return r; }
static long dot(const struct point *a, const struct point *b) { return (long)a->x * b->x + (long)a->y * b->y; }
static struct big make(long a, int k) {
  struct big r;
  r.a = a; r.b = (char)k; r.c = (short)(k * 3); r.d = k * 1.5; r.p.x = k; r.p.y = -k;
  return r;
}
static long use(struct big b) { return b.a + b.b + b.c + (long)b.d + b.p.x + b.p.y; }
int main(void) {
  struct point a = {3, 4}, b = {5, -6};
  struct point c = add(a, b);
  printf("%d %d %ld\n", c.x, c.y, dot(&a, &b));
  struct big big = make(1000000000000L, 7);
  printf("%ld %d %d %ld\n", big.a, big.b, big.c, use(big));
  struct big copy = big;
  copy.p.x = 99;
  printf("%d %d\n", big.p.x, copy.p.x);
  return 0;
}
