/* ON ERROR's landing on wasm32: setjmp always returns 0 and longjmp stops the program with a named failure of the
   host, so a program with a handler that would land in it ends there instead of continuing. Gorillas raises none
   (tests/wasm_gorillas.rs). */
#ifndef LLRM_WASM_SETJMP_H
#define LLRM_WASM_SETJMP_H
typedef int jmp_buf[1];

extern void llrm_host_fail(const char *message);

#define setjmp(env) ((void)(env), 0)
_Noreturn static inline void longjmp(jmp_buf env, int value)
{
    (void)env;
    (void)value;
    llrm_host_fail("ON ERROR landed: longjmp is not supported on wasm32");
    for (;;)
        ;
}
#endif
