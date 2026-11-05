# Sourced (sh or bash) by the A/B drivers: `ab_env_check || exit 2`.
# Refuses an LLRM_* variable outside AB_ENV_OK (tool locations) unless $AB_ALLOW_ENV names it; prints the others.
# A switch that changes the pipeline on one side gave a false second cause (LLRM_WASM_BACKEND_ONLY skipped the mid-end).
# llrmbin.assert_clean_env reads this list: keep it one quoted line.
AB_ENV_OK=" LLRM_ARM64_SYSROOT LLRM_BIN LLRM_CC LLRM_CC_DIR LLRM_CC_OPT LLRM_CC_REF_CLANG LLRM_CC_REF_FLAGS LLRM_CC_REF_LINK LLRM_CC_REF_TRIPLE LLRM_CC_RUN LLRM_CC_TARGET LLRM_CLANG_M16 LLRM_CLANG_WASM LLRM_EXPECT_SHA LLRM_LLC LLRM_MEMORY_LIMIT_KB LLRM_NO_BUILD LLRM_OW LLRM_PIN_BIN LLRM_PIN_CLANG LLRM_PIN_LLC LLRM_TRIPLE "

ab_env_check() {
  _lines=$(env | sort | grep '^LLRM_') || return 0
  _rc=0 _ifs=$IFS
  IFS='
'
  for _l in $_lines; do
    _n=${_l%%=*}
    case "$AB_ENV_OK${AB_ALLOW_ENV:+ $AB_ALLOW_ENV }" in
      *" $_n "*) echo "ab_env: env: $_l" >&2 ;;
      *) echo "ab_env: $_l changes the pipeline; unset it, or name it in AB_ALLOW_ENV if both sides are meant to run it" >&2; _rc=2 ;;
    esac
  done
  IFS=$_ifs
  return $_rc
}
