char a[1200], b[1200];

/* The early edge returns the induction cast to the return type. */
int first_index32(void) { for (int i = 0; i < 1200; i++) if (a[i] != b[i]) return i; return -1; }
short first_index16(void) { for (int i = 0; i < 1200; i++) if (a[i] != b[i]) return (short)i; return -1; }
