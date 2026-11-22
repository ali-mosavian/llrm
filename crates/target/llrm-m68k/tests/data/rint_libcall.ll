target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"
declare float @llvm.rint.f32(float)
declare double @llvm.rint.f64(double)
define float @rf(float %x) { %r = call float @llvm.rint.f32(float %x) ret float %r }
define double @rd(double %x) { %r = call double @llvm.rint.f64(double %x) ret double %r }
