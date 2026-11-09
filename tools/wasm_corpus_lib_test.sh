#!/usr/bin/env bash
# tools/wasm_corpus_lib_test.sh: the wasm matrix refuses to judge a row whose two sides ran different stubs.
. "$(dirname "$0")/wasm_corpus_lib.sh"
fail=0; t=$(mktemp -d); trap 'rm -rf "$t"' EXIT
check() { [ "$2" = "$3" ] || { echo "FAIL $1: got '$2', want '$3'"; fail=1; }; }
printf 'stub A' > "$t/a.o"; printf 'stub B' > "$t/b.o"
check same_stub_same_result "$(judge 7 7 "$(stub_md5 "$t/a.o")" "$(stub_md5 "$t/a.o")")" same
# The same result from two different stubs is not agreement: it was 0 cells compared on the arm64 flag fault.
check different_stub_refused "$(judge 7 7 "$(stub_md5 "$t/a.o")" "$(stub_md5 "$t/b.o")")" "n/a(stub differs: llrm $(stub_md5 "$t/a.o"), clang $(stub_md5 "$t/b.o"))"
check no_result_unjudged "$(judge none 7 x x)" "n/a(no result: llrm none, clang 7)"
check different_results "$(judge 7 8 x x)" "DIFFERENT(7/8)"
exit $fail
