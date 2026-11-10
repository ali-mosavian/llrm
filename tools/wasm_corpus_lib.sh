# Sourced by wasm_corpus.sh and its test.
# stub_md5 FILE: the stub object a side is linked with. Both sides must run the same bytes; a column from different stubs compares nothing.
stub_md5() { md5sum "$1" | cut -c1-12; }
# count OBJECT STUB: "result instructions stub_md5" of OBJECT linked with STUB and run. The md5 is of the copy that was linked and run, read
# back by this call, so a side that ran another stub reports it whatever path it was given.
count() {
  local linked="$1.stub.o" md5
  cp "$2" "$linked"; md5=$(stub_md5 "$linked")
  wasm-ld --no-entry --export=run --allow-undefined "$1" "$linked" -o "$1.wasm" 2>/dev/null || { echo "none 0 $md5"; return; }
  timeout 120 wasm-interp "$1.wasm" -r run --trace > "$1.trace" 2>&1
  echo "$(sed -n 's/^run() => i32:\(-*[0-9]*\)$/\1/p' "$1.trace" | head -1 | grep . || echo none) $(grep -c '^#' "$1.trace") $md5"
}
# judge LLRM_RESULT CLANG_RESULT LLRM_STUB_MD5 CLANG_STUB_MD5: "same", or why the row is not judged.
judge() {
  if [ "$3" != "$4" ]; then echo "n/a(stub differs: llrm $3, clang $4)"
  elif [ "${1:-none}" = none ] || [ "${2:-none}" = none ]; then echo "n/a(no result: llrm ${1:-none}, clang ${2:-none})"
  elif [ "$1" = "$2" ]; then echo same
  else echo "DIFFERENT($1/$2)"; fi
}
