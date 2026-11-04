#!/bin/bash
# usage: build.sh CC1 DIR LEVEL   (LEVEL = O2 | Os): compile the softfloat sources with the z80 cc1 and assemble them into DIR.
set -e
here=$(cd "$(dirname "$0")" && pwd); S=$here/../../../runtime/softfloat
CC1=$1; O=$2; L=$3; mkdir -p "$O"; ulimit -v 16000000
for f in sf32 sf32_conv sf32_6502 int32; do
  timeout 300 "$CC1" -quiet -"$L" -I"$here/inc" -I"$S/include" -DSF_INTABI "$S/src/$f.c" -o "$O/$f.s" || { echo "cc1 failed $f $L"; exit 1; }
  sdasz80 -o "$O/$f.rel" "$O/$f.s"
done
