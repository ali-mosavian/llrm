#!/bin/bash
# BASIC calling-convention matrix: BC and llrm-qb callers and callees.
#   [BCSW=switches] tools/callconv/bas.sh DIALECT WORK   (DIALECT: vbdos pds71 qb45)
# WORK gets BC's /A listings (*.LST), llrm-qb's -S text (*L.ASM), and one
# output per build: BB.OUT (BC caller, BC callee; the control), LB.OUT, BL.OUT,
# LL.OUT (first letter caller, second callee; L = llrm-qb). An .OUT that is
# missing means the run crashed or hung; see run.events.
set -eu
D=$1; W=$2
R="$(cd "$(dirname "$0")/../.." && pwd)"; B="${LLRM_BIN:-$R/target/release}"; SRC=$R/tests/fixtures/callconv/bas
TC=~/work/other/d32x/toolchains
DOSRUN=${DOSRUN:-$HOME/scratch/pr-dosbox/src/dosbox-x}
case $D in
  vbdos) T=$TC/vbdos; BC='v:\bin\bc'; LINK='v:\bin\link'; LIBF=VBDCL10E.LIB; SW='/O /FPi /Zi'; CUR="CUE CVE" ;;
  pds71) T=$TC/pds71; BC='v:\binb\bc'; LINK='v:\binb\link'; LIBF=BCL71ENR.LIB; SW='/O /FPi /G2 /Zi'; CUR="CUE CVE" ;;
  qb45)  T=~/work/42-labs/mini-qb/dosbox/qb45; BC='v:\bc'; LINK='v:\link'; LIBF=BCOM45.LIB; SW='/O /FPi /Zi'; CUR= ;;
esac
SW=${BCSW:-$SW}   # e.g. BCSW='/O /FPi /G3 /Zi'
rm -rf $W; mkdir -p $W; cp $SRC/*.BAS $SRC/*.BI $SRC/PROBE.ASM $W/
[[ $D == qb45 ]] && cp $SRC/qb45/*.BI $W/ && rm $W/CUE.BAS $W/CVE.BAS
cd $W
sed -i 's/$/\r/' *.BAS *.BI   # BC reads CRLF lines only
$B/jwasm -q -c -Zg -omf -FoPROBE.OBJ PROBE.ASM >/dev/null
$B/jwasm -q -c -Zg -omf -DNOARM -FoPROBEN.OBJ PROBE.ASM >/dev/null
CALLEES="CE CES $CUR"; CALLERS="CR CRS"
llrm() { # MODULE DIR: object and -S text into WORK
  $B/llrm-qb $2/$1.BAS --dialect $D --runtime $D -o $W/${1}L.OBJ 2> $W/${1}L.ERR || { rm -f $W/${1}L.OBJ; return 1; }
  $B/llrm-qb $2/$1.BAS --dialect $D --runtime $D -S -o $W/${1}L.ASM 2>/dev/null || true
}
mkdir -p nc; cp *.BAS *.BI nc/; cp $SRC/qb45/CUR.BI $SRC/qb45/CURCALL.BI nc/
for m in $CALLEES $CALLERS; do
  llrm $m $W && continue
  echo "llrm-qb $m: $(head -1 ${m}L.ERR)"
  # CURRENCY is declared in CC.BI, so every module meets it: retry without.
  [[ $m != CUE ]] && grep -q "CURRENCY\|CUR.BI" ${m}L.ERR && { llrm $m $W/nc && { echo "  built without the CURRENCY cases"; continue; }; echo "  without CURRENCY: $(head -1 ${m}L.ERR)"; }
  echo "  using BC's $m.OBJ in its place"
done
objs() { local o="" m; for m in "$@"; do [[ $L == 1 && -f ${m}L.OBJ ]] && o+="+${m}L.OBJ" || o+="+$m.OBJ"; done; echo ${o#+}; }
conf=job.conf
printf '[sdl]\nautolock=false\n[dosbox]\nmemsize=16\nstartbanner=false\nquit warning=false\n[cpu]\ncore=normal\ncycles=max\n' > $conf
link() { # EXE, caller objs, callee objs, probe
  echo "$LINK HN.OBJ+$2+$3+$4,$1,,$LIBF; > $1.LNK"; }
L=0; cr=$(objs $CALLERS); ce=$(objs $CALLEES); L=1; crl=$(objs $CALLERS); cel=$(objs $CALLEES)
{
  printf ':ms 300000\nmount w %s\nmount v %s\nset LIB=v:\\lib\nw:\n' "$W" "$T"
  for m in HN $CALLEES $CALLERS; do echo "$BC $m.BAS,$m.OBJ,$m.LST /A $SW; > $m.CC"; done
  link BB $cr $ce PROBE.OBJ
  link BL $cr $cel PROBE.OBJ
  link LB $crl $ce PROBEN.OBJ
  link LL $crl $cel PROBEN.OBJ
  printf '.\n'
  for x in BB BL LB LL; do printf ':ms 20000\nmount w %s\nw:\nif exist CC.OUT del CC.OUT\n%s\ncopy CC.OUT %s.OUT\n.\n' "$W" $x $x; done
} | DOSRUN_FD=3 SDL_VIDEODRIVER=dummy "$DOSRUN" -nolog -conf $conf 3>run.events >/dev/null 2>&1
for m in HN $CALLEES $CALLERS; do tr -d "\r" < $m.CC | grep -B1 "\^" | head -6; grep -H "Severe" $m.CC | grep -v " 0 Severe" || true; done
for x in BB BL LB LL; do grep -Hi "error" $x.LNK | head -5 || true; done
grep '"ev":"end"' run.events | cut -c1-150
for x in BB BL LB LL; do echo "== $x"; [[ -f $x.OUT ]] && tr -d '\r' < $x.OUT | head -60 || echo "(no output)"; done
