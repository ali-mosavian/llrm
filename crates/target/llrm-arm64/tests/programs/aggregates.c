int printf(const char *, ...);
struct pair { long a, b; };
struct triple { long a, b, c; };
struct vec2 { double x, y; };
struct mixed { int i; float f; };
struct tiny { char c; short s; };
static struct pair swap(struct pair p) { struct pair r = {p.b, p.a}; return r; }
static long sum3(struct triple t) { return t.a + t.b + t.c; }
static struct triple scale(struct triple t, long k) { struct triple r = {t.a * k, t.b * k, t.c * k}; return r; }
static struct vec2 add2(struct vec2 u, struct vec2 v) { struct vec2 r = {u.x + v.x, u.y + v.y}; return r; }
static struct mixed bump(struct mixed m) { m.i += 1; m.f *= 2; return m; }
static struct tiny shrink(struct tiny t) { t.c++; t.s += 2; return t; }
static long many(struct pair a, struct pair b, struct pair c, struct pair d, struct pair e) { return a.a + b.b + c.a + d.b + e.a; }
int main(void) {
  struct pair p = swap((struct pair){3, 4});
  printf("%ld %ld\n", p.a, p.b);
  struct triple t = scale((struct triple){1, 2, 3}, 5);
  printf("%ld %ld %ld %ld\n", t.a, t.b, t.c, sum3(t));
  struct vec2 v = add2((struct vec2){1.5, 2.5}, (struct vec2){3, 4});
  printf("%d %d\n", (int)(v.x * 10), (int)(v.y * 10));
  struct mixed m = bump((struct mixed){7, 1.25f});
  printf("%d %d\n", m.i, (int)(m.f * 100));
  struct tiny s = shrink((struct tiny){'a', 300});
  printf("%d %d\n", s.c, s.s);
  printf("%ld\n", many((struct pair){1, 2}, (struct pair){3, 4}, (struct pair){5, 6}, (struct pair){7, 8}, (struct pair){9, 10}));
  return 0;
}
