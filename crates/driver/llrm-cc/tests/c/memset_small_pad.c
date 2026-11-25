typedef struct { unsigned hash; unsigned char probe; } Verdict;
void set(Verdict *v, unsigned h, unsigned char p) { *v = (Verdict){h, p}; }
