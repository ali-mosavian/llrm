#!/bin/sh
# The binary128 entry points must have the signatures the pin's WebAssembly backend calls (WebAssemblyRuntimeLibcallSignatures.cpp): a value is two i64, a
# result comes back through an sret pointer, float/double stay f32/f64. Reference = the pin clang's own libcall declarations for C code on long double;
# ours = lib/fp/f128 built with. Before SF_QUADBITS the conversions took and returned i64 for double (INTABI): a link-valid but wrong call.
# usage: [LLRM_CLANG_WASM=pin clang with the WebAssembly target] tests/f128_wasm_abi.sh
cd "$(dirname "$0")/.." || exit 1
C=${LLRM_CLANG_WASM:-$HOME/scratch/llvm-build-clang-wasm/bin/clang}
T=$(mktemp -d); trap 'rm -r $T' EXIT; rc=0
cat > $T/ref.c <<'E2'
typedef long double q;
q add(q a, q b) { return a + b; } q sub(q a, q b) { return a - b; } q mul(q a, q b) { return a * b; } q div(q a, q b) { return a / b; }
int lt(q a, q b) { return a < b; } int le(q a, q b) { return a <= b; } int gt(q a, q b) { return a > b; } int ge(q a, q b) { return a >= b; }
int eq(q a, q b) { return a == b; } int ne(q a, q b) { return a != b; } int uo(q a, q b) { return __builtin_isunordered(a, b); }
q ext_f(float a) { return a; } q ext_d(double a) { return a; } float tr_f(q a) { return (float)a; } double tr_d(q a) { return (double)a; }
int fix_si(q a) { return (int)a; } unsigned fix_usi(q a) { return (unsigned)a; } long long fix_di(q a) { return (long long)a; } unsigned long long fix_udi(q a) { return (unsigned long long)a; }
__int128 fix_ti(q a) { return (__int128)a; } unsigned __int128 fix_uti(q a) { return (unsigned __int128)a; }
q fl_si(int a) { return a; } q fl_usi(unsigned a) { return a; } q fl_di(long long a) { return a; } q fl_udi(unsigned long long a) { return a; }
q fl_ti(__int128 a) { return a; } q fl_uti(unsigned __int128 a) { return a; }
E2
$C --target=wasm32 -O2 -S $T/ref.c -o - | grep -E '^\s*\.functype\s+__[a-z0-9]*tf[a-z0-9]*' | sort -u > $T/ref.txt
for f in $(tests/fp_sources.sh f128); do $C --target=wasm32 -O2 -Iinclude -S $f -o - | grep -E '^\s*\.functype\s+__[a-z0-9]*tf[a-z0-9]*' ; done | sort -u > $T/ours.txt
[ -s $T/ref.txt ] || { echo "no reference signatures (is $C a wasm32 clang?)"; exit 2; }
n=0; while read -r _ name sig; do n=$((n + 1)); grep -qF "$name $sig" $T/ours.txt || { echo "mismatch: pin calls $name $sig"; rc=1; }; done < $T/ref.txt
[ $rc = 0 ] && echo "f128 wasm abi ok ($n libcalls)"; exit $rc
