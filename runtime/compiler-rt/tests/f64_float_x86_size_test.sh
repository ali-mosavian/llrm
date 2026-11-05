#!/bin/sh
# usage: [LLRM_DIST=dir with llrm-cc and llrm-llc] tests/f64_float_x86_size_test.sh -> x86 m32 and m64 at -Os: floatsidf / floatunsidf, with the helpers they call, stay small and pull no pack.
# Before: both went through the generic from_mag and sf64_pack, alone m32 516 / 468 B, m64 311 / 262 B (llrm-cc, not system clang: its float.c is twice as large). With all four linked the direct form costs +23 / +33 B.
cd "$(dirname "$0")/.." || exit 2; T=$(mktemp -d); trap 'rm -r $T' EXIT
D=${LLRM_DIST:-$(python3 ../../tools/llrmbin.py bin 2> /dev/null)}
[ -x "$D/llrm-cc" ] && [ -x "$D/llrm-llc" ] || { echo "SKIPPED (not a pass): no llrm-cc/llrm-llc dist; set LLRM_DIST"; exit 2; }
export LLRM_LLC=$D/llrm-llc; bad=0
cat > $T/alone.py <<'P'
import re, subprocess, sys
obj, sym, pack_size = sys.argv[1], sys.argv[2], int(sys.argv[3])
size = {}
for l in subprocess.run(['nm', '-S', obj], capture_output=True, text=True).stdout.splitlines():
    p = l.split()
    if len(p) == 4 and p[2] in 'tT': size[p[3]] = int(p[1], 16)
dis = subprocess.run(['objdump', '-dr', '--no-show-raw-insn', obj], capture_output=True, text=True).stdout
body, cur = {}, None
for l in dis.splitlines():
    m = re.match(r'[0-9a-f]+ <(.+)>:', l)
    if m: cur = m.group(1); body[cur] = ''
    elif cur: body[cur] += l + '\n'
seen, todo, pack = set(), [sym], False
while todo:
    f = todo.pop()
    if f in seen: continue
    seen.add(f)
    for c in re.findall(r'<([A-Za-z_0-9]+)>', body.get(f, '')):
        if c in size: todo.append(c)
    if 'sf64_pack' in body.get(f, ''): pack = True
print(sum(size[f] for f in seen) + (pack_size if pack else 0))
P
check() { # target config sym bound
  (cd $PWD && LLRM_CC_TARGET=$1 $D/llrm-cc -Os -w -include stdint.h -include config/$2/config.h "-DSF_NAME(n)=sfb_##n" -Iinclude -c lib/fp/f64/float.c -o $T/f.o && LLRM_CC_TARGET=$1 $D/llrm-cc -Os -w -include stdint.h -include config/$2/config.h "-DSF_NAME(n)=sfb_##n" -Iinclude -c lib/fp/f64/pack.c -o $T/p.o) || { echo "FAIL: $1 did not compile"; bad=1; return; }
  ps=$(nm -S $T/p.o | awk '$3 ~ /[tT]/{print $2; exit}'); ps=$((0x$ps))
  n=$(python3 $T/alone.py $T/f.o sfb_$3 $ps)
  [ "$n" -le "$4" ] || { echo "FAIL: $1 $3 alone is $n B (bound $4)"; bad=1; }; }
check x86_32-linux x86_m32 floatsidf 130
check x86_32-linux x86_m32 floatunsidf 110
check x86_64-linux x86_m64 floatsidf 100
check x86_64-linux x86_m64 floatunsidf 85
[ $bad = 0 ] && echo "f64_float_x86_size ok"; exit $bad
