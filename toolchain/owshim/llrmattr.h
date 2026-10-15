/*
 * llrm: what gcc's __attribute__((...)) states that Open Watcom's front end has no word for. The target's header
 * (borland.h, flat.h) spells `__attribute__(list)` as `__declspec list`; ctype.c reads the list. These values sit
 * outside the enums of the unpatched headers they extend.
 */
#ifndef LLRMATTR_H
#define LLRMATTR_H

/* A bit of decl_info.decl_mod only: AdjModsTypeNode turns it into SYM_NOINLINE, so it is in no type. */
#define LLRM_FLAG_NOINLINE      0x00200000
#define LLRM_FLAG_ALWAYSINLINE  0x00400000
/* A bit of the symbol's flags (sym_flags stops at 0x40000): the function is never to be inlined. */
#define LLRM_SYM_NOINLINE       0x00080000
#define LLRM_SYM_ALWAYSINLINE   0x00100000
/* A bit of the call class (call_class stops at 0x400): the same, for the recorded stream. */
#define LLRM_FECALL_NOINLINE    0x00000800
#define LLRM_FECALL_ALWAYSINLINE 0x00001000

#endif
