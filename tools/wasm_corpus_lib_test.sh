#!/usr/bin/env bash
# tools/wasm_corpus_lib_test.sh: the wasm matrix reports the stub it linked and ran, and refuses to judge a row whose sides ran different stubs.
. "$(dirname "$0")/wasm_corpus_lib.sh"
fail=0; t=$(mktemp -d); trap 'rm -rf "$t"' EXIT
clang=${LLRM_CLANG_WASM:-$HOME/scratch/llvm-build-clang-wasm/bin/clang}
check() { [ "$2" = "$3" ] || { echo "FAIL $1: got '$2', want '$3'"; fail=1; }; }
echo 'int run(void) { return 7; }' > "$t/p.c"
echo 'int stub_value(void) { return 1; }' > "$t/a.c"; echo 'int stub_value(void) { return 2; }' > "$t/b.c"
for f in p a b; do "$clang" --target=wasm32 -ffreestanding -O2 -c "$t/$f.c" -o "$t/$f.o" || { echo "FAIL: clang cannot compile $f.c"; exit 2; }; done
cp "$t/p.o" "$t/ours.o"; cp "$t/p.o" "$t/pin.o"
read -r lv li lm <<< "$(count "$t/ours.o" "$t/a.o")"; read -r cv ci cm <<< "$(count "$t/pin.o" "$t/a.o")"
check same_stub_result "$lv" 7
check same_stub_md5_agrees "$lm" "$cm"
check same_stub_judged "$(judge "$lv" "$cv" "$lm" "$cm")" same
# Two runs of the same program, the pin's side given another stub: the same result is not agreement.
read -r cv ci cm <<< "$(count "$t/pin.o" "$t/b.o")"
check stub_md5_is_of_what_ran "$([ "$lm" != "$cm" ] && echo differs)" differs
check different_stub_refused "$(judge "$lv" "$cv" "$lm" "$cm")" "n/a(stub differs: llrm $lm, clang $cm)"
check no_result_unjudged "$(judge none 7 x x)" "n/a(no result: llrm none, clang 7)"
check different_results "$(judge 7 8 x x)" "DIFFERENT(7/8)"
exit $fail
