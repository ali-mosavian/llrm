#!/bin/bash
# O1/Os objects must be byte-identical under every rate (knob touches only rate-priced levels)
cd ~/scratch/w15-bar; D=$HOME/scratch/w15-idn/rstrict; mkdir -p $D
progs=$(ls ~/scratch/w15-out-halfbase1/*.cc.o | xargs -n1 basename | sed 's/\..*//' | sort -u)
for R in 62500 300 30000; do for l in 1 s; do mkdir -p $D/$R.$l; cp ~/work/personal/llrm/bench/bench_io.h $D/$R.$l/
  for p in $progs; do LLRM_INL_RATE=$R OUT=$D/$R.$l CC=$HOME/scratch/w15-idn/rr/llrm-cc ./build2.sh $p $l cc >/dev/null 2>&1; done; done; done
for l in 1 s; do for R in 300 30000; do n=0; for p in $progs; do cmp -s $D/62500.$l/$p.$l.cc.o $D/$R.$l/$p.$l.cc.o || { n=$((n+1)); echo "DIFF $R $l $p"; }; done; echo "rate $R level $l: $n differ"; done; done > $D/result.txt
touch $D/done
