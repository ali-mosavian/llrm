# Sourced by wasm_corpus.sh and its test.
# stub_md5 FILE: the stub object a side is linked with. Both sides must run the same bytes; a column from different stubs compares nothing.
stub_md5() { md5sum "$1" | cut -c1-12; }
# judge LLRM_RESULT CLANG_RESULT LLRM_STUB_MD5 CLANG_STUB_MD5: "same", or why the row is not judged.
judge() {
  if [ "$3" != "$4" ]; then echo "n/a(stub differs: llrm $3, clang $4)"
  elif [ "${1:-none}" = none ] || [ "${2:-none}" = none ]; then echo "n/a(no result: llrm ${1:-none}, clang ${2:-none})"
  elif [ "$1" = "$2" ]; then echo same
  else echo "DIFFERENT($1/$2)"; fi
}
