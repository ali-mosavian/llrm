extern void *grow(unsigned n);
#pragma aux grow = "sub sp,ax" __parm __nomemory [__ax] __value [__sp] __modify __exact __nomemory [__sp]
extern unsigned stackavail(void);
extern char *strcpy(char *, const char *);
typedef struct { int status; char name[16]; char msg[64]; } R;
extern char *errmsg[4];
extern unsigned memavl(void);
extern unsigned memmax(void);
void t(R *result)
{
    unsigned n, size, avail;
    char *buffer;
    size = memmax();
    if (memavl() < size) { result->status = 1; strcpy(result->msg, errmsg[0]); return; }
    avail = memavl();
    strcpy(result->name, "a");
    n = stackavail() + 1;
    buffer = grow(n);
    if (buffer != 0) { result->status = 1; strcpy(result->msg, errmsg[1]); return; }
    n = stackavail() >> 1;
    buffer = grow(n);
    if (buffer == 0) { result->status = 1; strcpy(result->msg, errmsg[2]); return; }
    if (memavl() < avail) { result->status = 1; strcpy(result->msg, errmsg[3]); }
    for (size = 0; size < n; ++size) buffer[size] = 6;
    result->status = 0;
}
