# pinfn32.sh FILE.c [extra cc flags] -> .text of ours vs pin llc (i486) on our final IR; fails if the IR carries SSE
F=$1; N=$(basename $F .c); D=${D:-$HOME/scratch/x87/tgt-tip/release}; S=$(mktemp -d /var/tmp/x87-pf32.XXXX); T=/tmp/x87_pf32_$N
LLRM_MIR_STAGES=$S LLRM_CC_TARGET=x86_32-linux LLRM_LLC=$D/llrm-llc $D/llrm-cc -std=c23 -${LV:-Os} -m32 -fno-pic -w -isystem /usr/include/x86_64-linux-gnu "${@:2}" -c $F -o $T.ours.o || exit 1
python3 - $(ls $S/*.ll | tail -1) $T.pin.ll <<'P' || exit 1
import re,sys
s=open(sys.argv[1]).read()
s=re.sub(r' "llrm[^"]*"="[^"]*"','',s); s=re.sub(r' llrm-[\w-]+','',s); s=re.sub(r' nearcode','',s)
s=re.sub(r'"(target-cpu|tune-cpu|target-features)"="[^"]*"\s*','',s)
s=re.sub(r'^(declare|define) (?!dso_local|internal|private)',lambda m:m.group(1)+' dso_local ',s,flags=re.M)
assert not re.search(r'\+sse|pentium4|"target-features"',s),'IR still carries SSE at m32'
open(sys.argv[2],'w').write(s)
P
${PIN_BIN:-$HOME/scratch/llvm-build/bin}/llc -O2 -relocation-model=static -mtriple=i386-linux-gnu -mcpu=i486 $T.pin.ll -filetype=obj -o $T.pinllc.o || exit 1
for v in ours pinllc; do echo $v $(size -A $T.$v.o | awk '$1==".text"{print $2}'); done
