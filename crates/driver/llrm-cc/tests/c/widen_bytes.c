void widen(unsigned short *restrict d, const unsigned char *restrict s) {
  for (int i = 0; i < 4096; i++) d[i] = s[i];
}
