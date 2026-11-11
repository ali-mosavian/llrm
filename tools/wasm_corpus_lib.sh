# Sourced by wasm_corpus.sh and its test.
# stub_md5 FILE: the stub object a side is linked with. Both sides must run the same bytes; a column from different stubs compares nothing.
stub_md5() { md5sum "$1" | cut -c1-12; }
# count OBJECT STUB: "result instructions stub_md5" of OBJECT linked with STUB and run. The md5 is of the copy that was linked and run, read
# back by this call, so a side that ran another stub reports it whatever path it was given.
count() {
  local linked="$1.stub.o" md5
  cp "$2" "$linked"; md5=$(stub_md5 "$linked")
  : > "$1.import"
  wasm-ld --no-entry --export=llrm_harness_run --allow-undefined "$1" "$linked" -o "$1.wasm" 2> "$1.ld" ||
    { sed -n 's/^wasm-ld: error: /link failed: /p' "$1.ld" | head -1 > "$1.import"; echo "none 0 $md5"; return; }
  # The trace counts instructions but costs minutes on a program of hundreds of millions (memfuncs_runtime: 4 s untraced, over 5 min
  # traced): past the cap the result comes from an untraced run and the count is "-", never 0.
  local status=0 instr
  # The trace is counted as it streams, never stored: a traced run writes gigabytes.
  : > "$1.trace"
  { timeout 60 wasm-interp "$1.wasm" -r llrm_harness_run --trace 2>&1; echo $? > "$1.status"; } |
    awk -v out="$1.trace" -v n="$1.count" '/^#/ { c++; next } { print > out } END { print c + 0 > n }'
  status=$(cat "$1.status"); instr=$(cat "$1.count")
  if [ "$status" -eq 124 ]; then
    instr=-; status=0
    timeout 120 wasm-interp "$1.wasm" -r llrm_harness_run > "$1.trace" 2>&1 || status=$?
    [ "$status" -ne 124 ] || echo "timed out after 120 s" > "$1.import"
  fi
  # An import no one satisfied stops the instance before it runs: its name goes to OBJECT.import, for the caller to report.
  [ -s "$1.import" ] || sed -n 's/.*invalid import "\([^"]*\)".*/unsatisfied import \1/p' "$1.trace" | head -1 > "$1.import"
  echo "$(sed -n 's/^llrm_harness_run() => i32:\(-*[0-9]*\)$/\1/p' "$1.trace" | head -1 | grep . || echo none) $instr $md5"
}
# judge LLRM_RESULT CLANG_RESULT LLRM_STUB_MD5 CLANG_STUB_MD5: "same", or why the row is not judged.
judge() {
  if [ "$3" != "$4" ]; then echo "n/a(stub differs: llrm $3, clang $4)"
  elif [ "${1:-none}" = none ] || [ "${2:-none}" = none ]; then echo "n/a(no result: llrm ${1:-none}, clang ${2:-none})"
  elif [ "$1" = "$2" ]; then echo same
  else echo "DIFFERENT($1/$2)"; fi
}
