int k(unsigned x) { int c = 0; do { c += x & 1; x >>= 1; } while (x != 0); return c; }
