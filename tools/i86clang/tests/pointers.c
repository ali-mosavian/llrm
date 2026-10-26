/* Arithmetic, comparison and conversion of pointers of each distance written out, so no model changes what they are. */
char far *far_add(char far *p, int i) { return p + i; }
int far_diff(char far *p, char far *q) { return q - p; }
int far_less(char far *p, char far *q) { return p < q; }
int far_same(char far *p, char far *q) { return p == q; }
char huge *huge_add(char huge *p, int i) { return p + i; }
long huge_diff(char huge *p, char huge *q) { return q - p; }
int huge_less(char huge *p, char huge *q) { return p < q; }
char far *to_far(char near *p) { return p; }
char near *to_near(char far *p) { return (char near *)p; }
int near_diff(char near *p, char near *q) { return q - p; }
int near_load(char near *p) { return *p; }
int far_load(char far *p) { return *p; }
