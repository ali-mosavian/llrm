extern long g;
void fill(int ch, int at) {
  char far *p = (char far *)g;
  int o;
  for (o = 0; o <= 3998; o += 2) { p[o] = ch + (o & 15); p[o + 1] = at; }
}
