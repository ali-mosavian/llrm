int printf(const char *, ...);
typedef unsigned long size_t;
size_t strlen(const char *);
static int sum(const int *p, int n) { int s = 0; for (int i = 0; i < n; i++) s += p[i]; return s; }
static void fill(int *p, int n) { for (int i = 0; i < n; i++) p[i] = i * i - 3 * i; }
static int mat[4][5];
static int trace(void) {
  for (int i = 0; i < 4; i++) for (int j = 0; j < 5; j++) mat[i][j] = i * 10 + j;
  int s = 0;
  for (int i = 0; i < 4; i++) s += mat[i][i + 1] * mat[3 - i][i];
  return s;
}
static char *copy(char *to, const char *from) { char *r = to; while ((*to++ = *from++)) {} return r; }
int main(void) {
  int data[20];
  fill(data, 20);
  printf("%d %d\n", sum(data, 20), sum(data + 5, 10));
  printf("%d\n", trace());
  char buffer[32];
  copy(buffer, "hello, arm64");
  printf("%s %d\n", buffer, (int)strlen(buffer));
  int *p = &data[7];
  int **pp = &p;
  **pp += 100;
  printf("%d %d %d\n", data[7], (int)(p - data), (int)(&data[19] - &data[3]));
  return 0;
}
