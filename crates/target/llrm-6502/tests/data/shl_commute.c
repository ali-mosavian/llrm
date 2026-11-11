int f(int *t, int at, int node, int a, int b) {
  int side = a < b ? 1 : 2;
  if (t[3 * at + side] == 0) t[3 * at + side] = node;
  return t[3*at+side];
}
