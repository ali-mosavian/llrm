; The intrinsics from clang's output that lli can run, run by lli and llrm
; alike; the rest are checked by hand in interpret_tests.rs.
; expect: 3
target datalayout = "e-p:16:16-p1:32:16:16:16-i32:16-i64:16"

declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)

define i32 @main() {
  %buf = alloca [4 x i8], align 1
  call void @llvm.lifetime.start.p0(i64 4, ptr %buf)
  call void @llvm.memset.p0.i16(ptr %buf, i8 3, i16 4, i1 false)
  %at = getelementptr inbounds i8, ptr %buf, i16 2
  %byte = load i8, ptr %at, align 1
  call void @llvm.lifetime.end.p0(i64 4, ptr %buf)
  %three = zext i8 %byte to i16
  %w = sext i16 %three to i32
  ret i32 %w
}
