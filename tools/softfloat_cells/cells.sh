#!/bin/bash
# The libgcc soft-fp column is NOT stock gcc: clang builds it with the machine modes defined by -D, abort() declared (sfpm-pre.h), rounding fixed to nearest, and for aarch64 an sfp-machine.h made at run time from the fetched tree (sfpm_aarch64.py: no fpcr read, S/D meats from config/riscv rv64; GCC's file is not copied here). Env: CRT (pin compiler-rt builtins dir), LIBGCC (gcc 15.2 libgcc dir), RUN (tests/run/c), KEEP=1 keeps the work dir.
# usage: cells.sh BASE_RT HEAD_RT prog... -> per program, aarch64 and riscv64 soft-float, library compiled -O2: ours base, ours head, compiler-rt (pin, -D__SOFTFP__), libgcc soft-fp (GCC 15.2 sources, clang): executed insns and linked .text; program output compared across columns.
BASE=$(cd "$1" && pwd); HEAD=$(cd "$2" && pwd); shift 2; H=$(cd "$(dirname "$0")" && pwd); W=$(mktemp -d); [ -n "$KEEP" ] || trap 'rm -rf "${W:?}"' EXIT; [ -n "$KEEP" ] && echo "KEEP $W"
C=/usr/bin/clang; CRT=${CRT:-$HOME/scratch/llvm-src/compiler-rt/lib/builtins}; S=${LIBGCC:-$HOME/scratch/sf-libgcc-src/libgcc}; RUN=${RUN:-$(cd "$H/../.." && pwd)/tests/run/c}
cat > $W/drv.c <<'D'
static char buf[4096]; static int len;
void report(long v) { char t[24]; int n = 0, neg = v < 0; unsigned long u = neg ? 0ul - (unsigned long)v : (unsigned long)v;
  do { t[n++] = '0' + u % 10; u /= 10; } while (u); if (neg) buf[len++] = '-'; while (n) buf[len++] = t[--n]; buf[len++] = '\n'; }
int main(void);
void *memcpy(void *d, const void *s, unsigned long n) { char *a = d; const char *b = s; while (n--) *a++ = *b++; return d; }
void *memset(void *d, int c, unsigned long n) { char *a = d; while (n--) *a++ = c; return d; }
int __fe_getround(void) { return 0; } int __fe_raise_inexact(void) { return 0; } void __sfp_handle_exceptions(int x) { (void)x; }
static void finish(void) {
#ifdef __riscv
  register long a0 __asm__("a0") = 1, a1 __asm__("a1") = (long)buf, a2 __asm__("a2") = len, a7 __asm__("a7") = 64;
  __asm__ volatile("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a7) : "memory");
  register long b0 __asm__("a0") = 0, b7 __asm__("a7") = 93; __asm__ volatile("ecall" : : "r"(b0), "r"(b7));
#else
  register long x0 __asm__("x0") = 1, x1 __asm__("x1") = (long)buf, x2 __asm__("x2") = len, x8 __asm__("x8") = 64;
  __asm__ volatile("svc 0" : "+r"(x0) : "r"(x1), "r"(x2), "r"(x8) : "memory");
  register long y0 __asm__("x0") = 0, y8 __asm__("x8") = 93; __asm__ volatile("svc 0" : : "r"(y0), "r"(y8));
#endif
}
void _start(void) { main(); finish(); for (;;) {} }
D
for tgt in aarch64-linux-gnu:"-mabi=aapcs-soft -mgeneral-regs-only -ffp-contract=off":qemu-aarch64:aarch64 riscv64-linux-gnu:"-march=rv64imac -mabi=lp64 -ffp-contract=off":qemu-riscv64:riscv64; do IFS=: read tc fl q cf <<<"$tgt"
  A=$W/$cf; mkdir -p $A/{base,head,crt,gcc}
  for side in base head; do rt=$BASE; [ $side = head ] && rt=$HEAD
    for f in $(cd $rt && sh tests/fp_sources.sh f32 f32_64 f64 int64) lib/int/clzsi2.c lib/int/clzdi2.c; do $C --target=$tc $fl -O2 -w -ffreestanding -include stdint.h -include $rt/config/$cf/config.h -I$rt/include -c $rt/$f -o $A/$side/$(echo $f | tr / _).o 2>/dev/null; done; ar rcs $A/$side.a $A/$side/*.o; done
  for f in $CRT/*df*.c $CRT/*sf*.c $CRT/clzsi2.c $CRT/clzdi2.c $CRT/divdi3.c $CRT/udivdi3.c $CRT/udivmoddi4.c $CRT/muldi3.c $CRT/ashldi3.c $CRT/lshrdi3.c; do $C --target=$tc $fl -O2 -w -ffreestanding -D__SOFTFP__ -I$CRT -c $f -o $A/crt/$(basename $f .c).o 2>/dev/null; done; ar rcs $A/crt.a $A/crt/*.o 2>/dev/null
  gi="-I$S/config/$cf"; gd=riscv; if [ $cf = aarch64 ]; then python3 "$H/sfpm_aarch64.py" "$S" "$A/sfpm-aarch64" || exit 2; gi="-I$A/sfpm-aarch64"; fi; [ $cf = riscv64 ] && gi="-I$S/config/riscv"; for f in $S/soft-fp/*df*.c $S/soft-fp/*sf*.c; do case $f in *hf*|*tf*|*xf*|*bf*) continue;; esac; $C --target=$tc $fl -O2 -w -ffreestanding -include $H/sfpm-pre.h -D__libgcc_cmp_return__=__SI__ -D__libgcc_word__=__DI__ -D__libgcc_unwind_word__=__DI__ $gi -I$S/soft-fp -I$S -I$S/../include -c $f -o $A/gcc/$(basename $f .c).o 2>/dev/null; done; ar rcs $A/gcc.a $A/gcc/*.o 2>/dev/null
  ar rcs $A/sup.a $A/base/lib_int_clzsi2.c.o $A/base/lib_int_clzdi2.c.o 2>/dev/null || ar rcs $A/sup.a $A/base/lib_int_clzsi2.o $A/base/lib_int_clzdi2.o
  echo "== $tc objects: base $(ls $A/base | wc -l) head $(ls $A/head | wc -l) compiler-rt $(ls $A/crt | wc -l) libgcc-softfp $(ls $A/gcc | wc -l)"
  $C --target=$tc $fl -O2 -w -ffreestanding -c $W/drv.c -o $A/drv.o
  for p in "$@"; do $C --target=$tc $fl -O2 -w -ffreestanding -c $RUN/$p.c -o $A/p.o || continue
    ref=; for col in base head crt gcc; do
      if ld.lld -static -e _start -o $A/x.elf $A/p.o $A/drv.o $A/$col.a $A/sup.a 2>$A/err; then :; else echo "$tc $p $col: link FAIL $(grep -m1 'undefined symbol' $A/err)"; continue; fi
      out=$($q $A/x.elf | md5sum | cut -c1-6); [ -z "$ref" ] && ref=$out
      echo "$tc $p $col: insns $(python3 $H/../../runtime/compiler-rt/tests/icount/count.py $q $A/x.elf) text $(size -A $A/x.elf | awk '$1 ~ /^\.text/{t+=$2}END{print t}') out $([ $out = $ref ] && echo same || echo DIFFERENT)"
    done
  done
done
