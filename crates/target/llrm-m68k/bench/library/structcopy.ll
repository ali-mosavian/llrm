target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @k(ptr initializes((0, 24)) %0, ptr %1) {
  tail call void @llvm.memcpy.p0.p0.i32(ptr align 4 dereferenceable(24) %0, ptr align 4 dereferenceable(24) %1, i32 24, i1 false)
  %3 = getelementptr i8, ptr %0, i32 8
  %4 = load i32, ptr %3, align 4
  %5 = getelementptr i8, ptr %0, i32 20
  %6 = load i8, ptr %5, align 4
  %7 = sext i8 %6 to i32
  %8 = add i32 %4, %7
  ret i32 %8
}

declare void @llvm.memcpy.p0.p0.i32(ptr , ptr , i32, i1 immarg)
