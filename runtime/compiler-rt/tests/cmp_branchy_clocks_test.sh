#!/bin/bash
# usage: LLRM=path/to/llrm-cc PYTHONPATH=dir-with-unicorn tests/cmp_branchy_clocks_test.sh
# x_sf_cmp64 (six f64 compares x 400 vectors, vsgcc 486 table) at -O2 took 95332 clk with the select-form compare core (gcc 88952);
# SF_CMP_BRANCHY in config/x86_m32 brings it to 89898. Bound 91000. Regenerates the kernel from this tree.
cd "$(dirname "$0")/.." || exit 2
: "${LLRM:?llrm-cc}"; V=../../crates/target/llrm-x86-m32/vsgcc; W=$(mktemp -d); trap 'rm -rf "${W:?}"' EXIT
bash tests/x86_kernels64.sh "$PWD" "$W/k" >/dev/null || exit 2
cp -r "$W/k/x_sf_cmp64" "$V/kernels/" && trap 'git checkout -q -- "$V/kernels/x_sf_cmp64"; rm -rf "${W:?}"' EXIT
export VSGCC_WORK=$W/w VSGCC_ONLY=llrm LLRM_BIN=$(dirname "$LLRM"); mkdir -p "$VSGCC_WORK"; cp "${VSGCC_STUB_DIR:?dir with stub.o stub.elf}"/stub.* "$VSGCC_WORK/"
(cd $V && ./build.sh x_sf_cmp64 >/dev/null && python3 harness.py x_sf_cmp64 llrm | tail -1) > $W/r || exit 2
n=$(python3 -c "import json,sys; d=json.load(open('$W/r')); print(d['clocks'] if d['ok'] else -1)")
[ "$n" -gt 0 ] && [ "$n" -le 91000 ] && echo "cmp_branchy_clocks ok ($n)" || { echo "FAIL: x_sf_cmp64 -O2 is $n clk (bound 91000)"; exit 1; }
