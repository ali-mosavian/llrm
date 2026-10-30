int printf(const char *, ...);
static unsigned char s[100000];
static int hist[256];
int main(void) {
  int n = 0;
  for (int i = 2; i < 100000; i++) s[i] = 1;
  for (int i = 2; i * i < 100000; i++) if (s[i]) for (int j = i * i; j < 100000; j += i) s[j] = 0;
  for (int i = 2; i < 100000; i++) if (s[i]) { n++; hist[i & 255]++; }
  int best = 0; for (int i = 0; i < 256; i++) if (hist[i] > hist[best] || (hist[i] == hist[best] && i < best)) best = i;
  printf("%d %d\n", n, best);
  return 0;
}
