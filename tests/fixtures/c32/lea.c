int lea_sum(int a, int b, int n) { int s = a + b; int t = a - b; if (s < n) return t; return s + t; }
