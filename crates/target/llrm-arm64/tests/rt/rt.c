/* A freestanding C runtime for AArch64 Linux, enough for the test programs:
   _start, exit, write, putchar, puts, printf (%d %i %u %x %c %s %ld %lu %lx %%
   with a width and a 0 flag), strlen, memcpy, memset. It is built by the pinned
   clang, so a program's output is not the compiler under test's own. */
typedef unsigned long size_t;
typedef __builtin_va_list va_list;
#define va_start(a, b) __builtin_va_start(a, b)
#define va_arg(a, t) __builtin_va_arg(a, t)
#define va_end(a) __builtin_va_end(a)

static long syscall3(long number, long a, long b, long c) {
  register long x8 __asm__("x8") = number;
  register long x0 __asm__("x0") = a;
  register long x1 __asm__("x1") = b;
  register long x2 __asm__("x2") = c;
  __asm__ volatile("svc #0" : "+r"(x0) : "r"(x8), "r"(x1), "r"(x2) : "memory");
  return x0;
}

void exit(int status) {
  syscall3(94, status, 0, 0);
  for (;;) {}
}

long write(int fd, const void *data, size_t size) { return syscall3(64, fd, (long)data, (long)size); }

size_t strlen(const char *s) {
  size_t n = 0;
  while (s[n]) n++;
  return n;
}

void *memcpy(void *to, const void *from, size_t n) {
  char *d = to;
  const char *s = from;
  while (n--) *d++ = *s++;
  return to;
}

void *memset(void *to, int value, size_t n) {
  char *d = to;
  while (n--) *d++ = (char)value;
  return to;
}

int putchar(int c) {
  char byte = (char)c;
  write(1, &byte, 1);
  return c;
}

int puts(const char *s) {
  write(1, s, strlen(s));
  putchar('\n');
  return 1;
}

static void number(unsigned long value, int base, int negative, int width, int zero) {
  char buffer[24];
  int at = 24;
  do {
    int digit = (int)(value % (unsigned long)base);
    buffer[--at] = (char)(digit < 10 ? '0' + digit : 'a' + digit - 10);
    value /= (unsigned long)base;
  } while (value);
  if (negative) buffer[--at] = '-';
  for (int pad = width - (24 - at); pad > 0; pad--) putchar(zero ? '0' : ' ');
  write(1, buffer + at, (size_t)(24 - at));
}

int printf(const char *format, ...) {
  va_list arguments;
  va_start(arguments, format);
  for (; *format; format++) {
    if (*format != '%') {
      putchar(*format);
      continue;
    }
    format++;
    int zero = 0, width = 0, wide = 0;
    if (*format == '0') { zero = 1; format++; }
    while (*format >= '0' && *format <= '9') width = width * 10 + (*format++ - '0');
    while (*format == 'l') { wide++; format++; }
    switch (*format) {
      case 'd': case 'i': {
        long v = wide ? va_arg(arguments, long) : va_arg(arguments, int);
        number(v < 0 ? 0UL - (unsigned long)v : (unsigned long)v, 10, v < 0, width, zero);
        break;
      }
      case 'u': number(wide ? va_arg(arguments, unsigned long) : va_arg(arguments, unsigned), 10, 0, width, zero); break;
      case 'x': number(wide ? va_arg(arguments, unsigned long) : va_arg(arguments, unsigned), 16, 0, width, zero); break;
      case 'c': putchar(va_arg(arguments, int)); break;
      case 's': { const char *s = va_arg(arguments, const char *); write(1, s, strlen(s)); break; }
      case '%': putchar('%'); break;
      default: putchar('?'); break;
    }
  }
  va_end(arguments);
  return 0;
}

static char heap[1 << 20];
static unsigned long heap_top;

void *malloc(size_t n) {
  heap_top = (heap_top + 15) & ~15ul;
  void *p = heap + heap_top;
  heap_top += n;
  return p;
}

void free(void *p) { (void)p; }

void *calloc(size_t n, size_t m) { return memset(malloc(n * m), 0, n * m); }

int strcmp(const char *a, const char *b) {
  while (*a && *a == *b) { a++; b++; }
  return (unsigned char)*a - (unsigned char)*b;
}

char *strcpy(char *to, const char *from) {
  char *r = to;
  while ((*to++ = *from++)) {}
  return r;
}

int memcmp(const void *a, const void *b, size_t n) {
  const unsigned char *x = a, *y = b;
  for (; n; n--, x++, y++) if (*x != *y) return *x - *y;
  return 0;
}

int atoi(const char *s) {
  int sign = 1, v = 0;
  if (*s == '-') { sign = -1; s++; }
  while (*s >= '0' && *s <= '9') v = v * 10 + (*s++ - '0');
  return sign * v;
}

void qsort(void *base, size_t n, size_t size, int (*cmp)(const void *, const void *)) {
  char *a = base;
  char tmp[64];
  for (size_t i = 1; i < n; i++) {
    for (size_t j = i; j > 0 && cmp(a + (j - 1) * size, a + j * size) > 0; j--) {
      memcpy(tmp, a + (j - 1) * size, size);
      memcpy(a + (j - 1) * size, a + j * size, size);
      memcpy(a + j * size, tmp, size);
    }
  }
}

int main(void);

void _start(void) { exit(main()); }
