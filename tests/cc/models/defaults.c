/* What an unmarked pointer is under each memory model, and what Borland's __near__/__far__/__huge__ make of one. */
char *np;
char __far__ *fp;
char __huge__ *hp;
int (*nf)(void);
int (__far__ *ff)(void);
int (__near__ *nn)(void);
struct S { char *p; int (*f)(void); int k; };
unsigned s_np = sizeof(np), s_fp = sizeof(fp), s_hp = sizeof(hp);
unsigned s_nf = sizeof(nf), s_ff = sizeof(ff), s_nn = sizeof(nn);
unsigned s_int = sizeof(int), s_size = sizeof(sizeof(int)), s_diff = sizeof(np - np), s_struct = sizeof(struct S);
int plain(void) { return 1; }
int c_far(void) { return ff(); }
int c_near(void) { return nn(); }
int c_def(void) { nf = plain; ff = plain; nn = plain; return nf(); }
