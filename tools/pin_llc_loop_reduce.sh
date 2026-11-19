#!/bin/bash
# Stands in for the pin's `opt -S -passes=loop-reduce -mtriple=T FILE` where the pin build has no `opt`:
# llc's own loop-reduce on the same IR and target, printed. PIN_LLC overrides the llc.
for a in "$@"; do case $a in -mtriple=*) T=${a#-mtriple=};; -*) ;; *) F=$a;; esac; done
"${PIN_LLC:-$HOME/work/other/llvm-pin/build/bin/llc}" -O2 -mtriple="$T" -print-after=loop-reduce -print-module-scope -o /dev/null "$F" 2>&1 | sed -n '/^define/,/^}/p'
