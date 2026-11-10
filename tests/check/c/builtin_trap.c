// RUN: llrm-cc %s -O2 -w -march=i386 -S -o /dev/stdout
// `__builtin_trap()` is `llvm.trap`: llrm-llc panicked on it at every x86 target (assemble.rs, no entry for the
// declaration). It is a `ud2` (0F 0B) and nothing after it.
// CHECK-LABEL: {{_t(@3)?}} proc
// CHECK: {{ud2|db 00fh,00bh}}
// CHECK: {{_t(@3)?}} endp
void t(void)
{
    __builtin_trap();
}
