/* The C calling-convention matrix, compiled alike by BCC 3.1 and llrm-c.
   A convention file defines CONV (cdecl or pascal) and DIST (far or near)
   and includes this. */
typedef struct { char b[1]; } S1;
typedef struct { char b[2]; } S2;
typedef struct { char b[3]; } S3;
typedef struct { char b[4]; } S4;
typedef struct { char b[5]; } S5;
typedef struct { char b[7]; } S7;
typedef struct { char b[8]; } S8;
typedef struct { char b[9]; } S9;
typedef struct { char c; int i; } SCI;

#define NARROW(X) X(sc, signed char) X(uc, unsigned char) X(ch, char) X(i, int) X(u, unsigned)
#define SCALARS(X) NARROW(X) X(l, long) X(ul, unsigned long) X(np, char near *) \
    X(fp, char far *) X(hp, char huge *) X(f, float) X(d, double) X(ld, long double)
#define AGGREGATES(X) X(s1, S1) X(s2, S2) X(s3, S3) X(s4, S4) X(s5, S5) X(s7, S7) \
    X(s8, S8) X(s9, S9) X(sci, SCI)
/* What a variadic callee reads each type back as, promoted. */
#define PROMOTED(X) X(sc, signed char, int) X(uc, unsigned char, int) X(ch, char, int) \
    X(i, int, int) X(u, unsigned, unsigned) X(l, long, long) X(ul, unsigned long, unsigned long) \
    X(np, char near *, char near *) X(fp, char far *, char far *) X(f, float, double) \
    X(d, double, double) X(ld, long double, long double) X(s3, S3, S3) X(s5, S5, S5)

/* Borland's STDARG.H, medium model. */
typedef void *va_list;
#define __size(x) ((sizeof(x) + sizeof(int) - 1) & ~(sizeof(int) - 1))
#define va_start(ap, parmN) ((void)((ap) = (va_list)((char *)(&parmN) + __size(parmN))))
#define va_arg(ap, type) (*(type *)(((*(char **)&(ap)) += __size(type)) - (__size(type))))

/* Clobbers every register a callee may: EAX-EDX, ES, and the high halves
   of ESI, EDI and EBP. */
extern void far trash(void);

#define INPUT(n, T) extern T in_##n;
SCALARS(INPUT) AGGREGATES(INPUT)
#undef INPUT
