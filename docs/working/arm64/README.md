# llrm-cc on the QB host runtime (arm64-macos): 14 of 74 sources fail
Repro: LLRM_CC_TARGET=arm64-macos LLRM_PIN_CLANG=<pin>/bin/clang target/release/llrm-cc -O2 -std=gnu11 -w -D__cdecl= \
  -I runtime/qb/posix -I runtime/qb -I runtime/qb/dev -I runtime/qb/dev/sdl -I runtime/qb/dev/host -I runtime/shared/posix \
  -I <SDL2>/include -c FILE.c -o FILE.o     (build script: gor/build.sh, in this directory)
Result: GORILLAS runs (headless device) with these 14 built by system clang and the other 60 by llrm-cc.

## (a) `appending` linkage, @llvm.global_ctors (__attribute__((constructor)))
Error: llrm-arm64-llc: line N: expected `global` or `constant`, found `appending`
Files: runtime/qb/{array,cngfx,cntext,file,gfx,nheap,onerror,play}.c
Minimal: nheap.ll line 10: @llvm.global_ctors = appending global [1 x { i32, ptr, ptr }] [{ i32, ptr, ptr } { i32 65535, ptr @xi_run_nh_xinit, ptr null }]
Needs: parse `appending`, and run the ctors (Mach-O __mod_init_func).

## (b) C11 atomics
Error: llrm-arm64-llc: line N: expected a type, found `atomic`   (present.c: `atomicrmw` is not an instruction of MIR's subset)
Files: runtime/qb/dev/sdl/{audio,host,input,present}.c, runtime/qb/dev/host/{video,tick}.c

## (c) Mach-O objects carry no LC_BUILD_VERSION
ld: warning: no platform load command found in '<obj>', assuming: macOS   (every llrm-cc and llrm-qb --arm64=darwin object)
