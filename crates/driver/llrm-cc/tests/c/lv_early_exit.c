unsigned char a[1200], b[1200];
int differ_arrays(void) {
  for (int i = 0; i < 1200; i++)
    if (a[i] != b[i]) return 1;
  return 0;
}
