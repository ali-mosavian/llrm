int printf(const char *, ...);
typedef unsigned long size_t;
void qsort(void *, size_t, size_t, int (*)(const void *, const void *));
int strcmp(const char *, const char *);
int atoi(const char *);
struct item { const char *name; int score; };
static int by_score(const void *a, const void *b) { return ((const struct item *)b)->score - ((const struct item *)a)->score; }
static int by_name(const void *a, const void *b) { return strcmp(((const struct item *)a)->name, ((const struct item *)b)->name); }
static int ints(const void *a, const void *b) { return *(const int *)a - *(const int *)b; }
int main(void) {
  struct item items[] = {{"delta", 40}, {"alpha", 90}, {"charlie", 10}, {"bravo", 75}, {"echo", 60}};
  qsort(items, 5, sizeof items[0], by_score);
  for (int i = 0; i < 5; i++) printf("%s:%d ", items[i].name, items[i].score);
  printf("\n");
  qsort(items, 5, sizeof items[0], by_name);
  for (int i = 0; i < 5; i++) printf("%s ", items[i].name);
  printf("\n");
  int v[12];
  for (int i = 0; i < 12; i++) v[i] = atoi("-37") * (i % 5) + i * 13 % 17;
  qsort(v, 12, sizeof v[0], ints);
  for (int i = 0; i < 12; i++) printf("%d ", v[i]);
  printf("\n");
  return 0;
}
