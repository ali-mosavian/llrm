/* The multiply that scales an index to bytes: of an unsigned index, however it
   arrives, it does not wrap unsigned; of a signed one, C promises nothing. */
int __far *base;
long through_a_char(int __far *a, char c) { return a[(unsigned)c]; }
long through_a_sum(int __far *a, unsigned u) { return a[u + 1]; }
long through_a_signed(int __far *a, int i) { return a[i]; }
