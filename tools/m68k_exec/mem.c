void *memset(void *p, int c, unsigned long n){ unsigned char *q=p; while(n--) *q++=(unsigned char)c; return p; }
void *memcpy(void *d, const void *s, unsigned long n){ unsigned char *a=d; const unsigned char *b=s; while(n--) *a++=*b++; return d; }
void *memmove(void *d, const void *s, unsigned long n){ unsigned char *a=d; const unsigned char *b=s; if(a<b) while(n--) *a++=*b++; else { a+=n; b+=n; while(n--) *--a=*--b; } return d; }
int memcmp(const void *x, const void *y, unsigned long n){ const unsigned char *a=x,*b=y; for(;n;n--,a++,b++) if(*a!=*b) return *a-*b; return 0; }
