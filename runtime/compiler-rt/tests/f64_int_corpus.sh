#!/bin/sh
# usage: tests/f64_int_corpus.sh BASE_RT_DIR [program...] -> tests/run/c int<->float programs, system clang -ffreestanding soft-float for aarch64 and riscv64,
# linked against lib/fp of BASE_RT_DIR and of this tree, EXECUTED under qemu-user: output equal between base and head, image md5 and .text size of both.
# aarch64/riscv64 are not llrm-cc targets (the FPU is used there), so this is the corpus A/B instrument for them. `long` is 64-bit here: outputs are compared base vs head;
# KEEP=dir keeps the images (for tools/ab_guard.sh). The .out files (32-bit long) are compared too and a difference is printed as "differs from .out" (expected for programs that print long sums).
HERE=$(cd "$(dirname "$0")/.." && pwd); BASE=$(cd "$1" && pwd) || exit 2; shift; C=${CLANG:-/usr/bin/clang}; T=$(mktemp -d); trap 'rm -r $T' EXIT
RUN=$HERE/../../tests/run/c
progs=${*:-"constant_int_float_compare double_against_int float_conversions_loop double_to_int64 float_bits float_return float_in_memory_loop float_in_place_update int_compared_with_float uint64_to_float double_across_a_call_result fcmp_all zz_f"}
cat > $T/drv.c <<'D'
static char buf[4096]; static int len;
void report(long v) { char t[24]; int n = 0, neg = v < 0; unsigned long u = neg ? 0ul - (unsigned long)v : (unsigned long)v;
  do { t[n++] = '0' + u % 10; u /= 10; } while (u); if (neg) buf[len++] = '-'; while (n) buf[len++] = t[--n]; buf[len++] = '\n'; }
int main(void);
void *memcpy(void *d, const void *s, unsigned long n) { char *a = d; const char *b = s; while (n--) *a++ = *b++; return d; }
void *memset(void *d, int c, unsigned long n) { char *a = d; while (n--) *a++ = c; return d; }
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
lib() { # rt triple flags out.a : lib/fp + int64 as an archive, so a program pulls only the routines it calls
  rt=$1; d=$T/lib.$(basename $4 .a); rm -rf $d; mkdir -p $d; srcs="$(cd $rt && sh tests/fp_sources.sh f32 f32_64 f64 int64) lib/int/clzsi2.c lib/int/clzdi2.c"
  cfg=$(case $2 in aarch64*) echo aarch64;; *) echo riscv64;; esac)
  for f in $srcs; do $C --target=$2 $3 -O2 -w -ffreestanding -include stdint.h -include $rt/config/$cfg/config.h -I$rt/include -c $rt/$f -o $d/$(echo $f | tr / _).o || return 1; done
  rm -f $4; ar rcs $4 $d/*.o; }
link() { # triple flags lib.a out prog opt
  $C --target=$1 $2 $6 -w -ffreestanding -c $RUN/$5.c -o $T/prog.o && $C --target=$1 $2 -O2 -w -ffreestanding -c $T/drv.c -o $T/drv.o && ld.lld -static -o $3.elf $T/prog.o $T/drv.o $3; }
bad=0
for tgt in aarch64-linux-gnu:"-mabi=aapcs-soft -mgeneral-regs-only -ffp-contract=off" riscv64-linux-gnu:"-march=rv64imac -mabi=lp64 -ffp-contract=off"; do tc=${tgt%%:*}; fl=${tgt#*:}; lib $BASE $tc "$fl" $T/base.a 2>$T/err && lib $HERE $tc "$fl" $T/head.a 2>>$T/err || { echo "$tc: runtime did not build: $(head -c 300 $T/err)"; bad=1; continue; }; q=$(case $tc in aarch64*) echo qemu-aarch64;; *) echo qemu-riscv64;; esac)
 for p in $progs; do for opt in -O2 -Os; do
  if ! link $tc "$fl" $T/base.a x $p $opt 2> $T/err || ! cp $T/base.a.elf $T/b.elf || ! link $tc "$fl" $T/head.a x $p $opt 2>> $T/err || ! cp $T/head.a.elf $T/h.elf; then echo "$tc $p $opt: not linked ($(grep -m1 -o 'undefined symbol: [^ ]*\|error: .*' $T/err | head -1))"; continue; fi
  $q $T/b.elf > $T/b.out; rb=$?; $q $T/h.elf > $T/h.out; rh=$?
  mb=$(md5sum < $T/b.elf | cut -c1-8); mh=$(md5sum < $T/h.elf | cut -c1-8)
  sb=$(size -A $T/b.elf | awk '$1 ~ /^\.text/{t+=$2}END{print t}'); sh_=$(size -A $T/h.elf | awk '$1 ~ /^\.text/{t+=$2}END{print t}')
  [ -n "$KEEP" ] && { mkdir -p $KEEP; cp $T/b.elf $KEEP/$tc.$p$opt.base; cp $T/h.elf $KEEP/$tc.$p$opt.head; }
  same=same; cmp -s $T/b.out $T/h.out || { same=DIFFERENT; bad=1; }; [ $rb = $rh ] || { same=DIFFERENT; bad=1; }
  ref=; cmp -s $T/h.out $RUN/$p.out || ref=" (differs from .out)"
  echo "$tc $p $opt: exec $same rc $rb/$rh md5 $mb/$mh text $sb/$sh_$ref"
 done; done; done
exit $bad
