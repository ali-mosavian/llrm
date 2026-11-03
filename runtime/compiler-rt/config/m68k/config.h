/* The m68k configuration for the 68020 and later, which have a 32/32 divide (divu.l): division in four hardware divides. The 68000 and 68010 keep the
 * restoring loop (their divide is 32/16 and a 32-bit `/` is a library call). Include with -include. */
#ifndef SF_M68K_H
#define SF_M68K_H
#define SF_FIX_NAN_UNDEF 1 /* NaN to int is undefined in C and poison in IR: -O2 fix_si 39 -> 34, fix_usi 34 -> 30, fix_di 197 -> 181 asm lines */
#ifdef __mc68020__
#define SF_DIV_HW32 1
#define SF_SPECIALS_ONCE 1 /* 68020: mul -4.3% executed instructions, text -20 B; on the 68000 it costs 18..30 B */
#endif
#endif
