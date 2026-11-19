int k(int n) { int a = 0, b = 1, s, i = 0; do { s = a + b; a = b; b = s; i++; } while (i < n); return s; }
