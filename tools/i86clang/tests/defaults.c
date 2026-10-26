/* What an unmarked pointer is under each memory model, and what Borland's near/far/huge make of one. */
char *np;
char far *fp;
char huge *hp;
int (*nf)(void);
int (far *ff)(void);
int (near *nn)(void);
struct S { char *p; int (*f)(void); int k; };
unsigned s_np = sizeof(np), s_fp = sizeof(fp), s_hp = sizeof(hp);
unsigned s_nf = sizeof(nf), s_ff = sizeof(ff), s_nn = sizeof(nn);
unsigned s_int = sizeof(int), s_size = sizeof(sizeof(int)), s_diff = sizeof(np - np), s_struct = sizeof(struct S);
int plain(void) { return 1; }
int c_far(void) { return ff(); }
int c_near(void) { return nn(); }
int c_def(void) { nf = plain; ff = plain; nn = plain; return nf(); }
