int printf(const char *, ...);
static int cols[12], d1[24], d2[24];
static int solve(int row, int n) {
  if (row == n) return 1;
  int count = 0;
  for (int c = 0; c < n; c++) {
    if (cols[c] || d1[row + c] || d2[row - c + n]) continue;
    cols[c] = d1[row + c] = d2[row - c + n] = 1;
    count += solve(row + 1, n);
    cols[c] = d1[row + c] = d2[row - c + n] = 0;
  }
  return count;
}
static long ack(long m, long n) { return m == 0 ? n + 1 : n == 0 ? ack(m - 1, 1) : ack(m - 1, ack(m, n - 1)); }
static unsigned long long mulhi(unsigned long a, unsigned long b) { return (unsigned long)(((__uint128_t)a * b) >> 64); }
int main(void) {
  for (int n = 4; n <= 8; n++) printf("%d ", solve(0, n));
  printf("\n%ld %lu\n", ack(2, 3), (unsigned long)mulhi(0xdeadbeefcafef00dul, 0x123456789abcdef1ul));
  return 0;
}
