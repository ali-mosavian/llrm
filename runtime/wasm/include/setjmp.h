/* ON ERROR's landing on wasm32: setjmp always returns 0 and longjmp traps, so a program with a handler that
   would land in it stops with an unreachable instead of continuing. Gorillas does not raise one. */
#ifndef LLRM_WASM_SETJMP_H
#define LLRM_WASM_SETJMP_H
typedef int jmp_buf[1];

#define setjmp(env) ((void)(env), 0)
_Noreturn static inline void longjmp(jmp_buf env, int value)
{
    (void)env;
    (void)value;
    __builtin_trap();
}
#endif
