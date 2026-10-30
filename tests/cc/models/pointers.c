/* Arithmetic, comparison and conversion of pointers of each distance written out, so no model changes what they are. */
char __far__ *far_add(char __far__ *p, int i) { return p + i; }
int far_diff(char __far__ *p, char __far__ *q) { return q - p; }
int far_less(char __far__ *p, char __far__ *q) { return p < q; }
int far_same(char __far__ *p, char __far__ *q) { return p == q; }
char __huge__ *huge_add(char __huge__ *p, int i) { return p + i; }
long huge_diff(char __huge__ *p, char __huge__ *q) { return q - p; }
int huge_less(char __huge__ *p, char __huge__ *q) { return p < q; }
char __far__ *to_far(char __near__ *p) { return p; }
char __near__ *to_near(char __far__ *p) { return (char __near__ *)p; }
int near_diff(char __near__ *p, char __near__ *q) { return q - p; }
int near_load(char __near__ *p) { return *p; }
int far_load(char __far__ *p) { return *p; }
