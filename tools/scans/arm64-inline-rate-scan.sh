#!/bin/bash
cd ~/scratch/w15-idn; P=$HOME/scratch/sf-venv/bin/python3
ALL="bintree crc fib floats fpbench frames hanoi histo jpeg_fixed jpeg_single lru mandel matmul nbody nbody_fixed nbody_single particle queens quicksort recchop recfloat recgcd recmany recpow recsum rectwo ring scroll shellsort sieve stft_fixed stft_single switch tile"
for R in 62500 300 3000 30000 10000; do for L in 2 3; do
  cells=""; for c in $ALL; do cells="$cells $c:$L"; done
  $HOME/scratch/lg/load_guard.sh env LLRM_INL_RATE=$R nice -n10 $P cc_ab2.py rs$R$L $HOME/scratch/w15-idn/rr/llrm-cc $cells
done; done
touch ~/scratch/w15-idn/rscan.done
