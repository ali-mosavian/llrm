#!/bin/bash
# side.sh PROG LEVEL: post-mid-end IR of both pipelines, and both listings, to $VSGCC_WORK/side/
W=$VSGCC_WORK; o=$W/side/$1.$2; mkdir -p $o
cp $W/db/$1.$2/07-available-externally.ll $o/ours.ll
cp $W/d-${V:-llvm}/$1.$2/03-available-externally.ll $o/llvm.ll
cp $W/db/$1.$2/listing.asm $o/ours.asm; cp $W/d-${V:-llvm}/$1.$2/listing.asm $o/llvm.asm
