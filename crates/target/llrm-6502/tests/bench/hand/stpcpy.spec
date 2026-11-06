name: stpcpy
hand: _stpcpy
kernel: k_stpcpy
args: P(dst,0),P(str,0)
wide: 1
rel: dst
expect: 11
note: copies src with its terminator to dst, returns a pointer to the copied terminator; links strcpy.s too (stpcpy jumps into the new strcpy's Y/ptr2 contract)
deps: strcpy      # linked with it: stpcpy jumps into the new strcpy
