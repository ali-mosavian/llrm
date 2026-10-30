target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" memory(inaccessiblemem: readwrite) {
b1:
  %0 = alloca [8192 x i8]
  br label %b3

b3:
  %1 = phi i16 [ %4, %b3 ], [ -8192, %b1 ]
  %2 = getelementptr i8, ptr %0, i16 %1
  %3 = getelementptr i8, ptr %2, i16 8192
  call void @llvm.memset.p0.i16(ptr align 2 %3, i8 0, i16 128, i1 false)
  %4 = add nsw i16 %1, 128
  %exitcond1 = icmp eq i16 %4, 0
  br i1 %exitcond1, label %5, label %b3

5:
  br label %b11

b11:
  %6 = phi ptr [ %0, %5 ], [ %9, %b11 ]
  %7 = phi i16 [ 64, %5 ], [ %8, %b11 ]
  call void @llvm.experimental.memset.pattern.p0.i16.i16(ptr nonnull %6, i16 7, i16 64, i1 false)
  %8 = add nsw i16 %7, -1
  %9 = getelementptr i8, ptr %6, i16 128
  %exitcond = icmp eq i16 %8, 0
  br i1 %exitcond, label %b13, label %b11

b13:
  %10 = getelementptr inbounds nuw i8, ptr %0, i16 8190
  %11 = load i16, ptr %10, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$PI2(i16 %11)
  tail call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.experimental.memset.pattern.p0.i16.i16(ptr nocapture writeonly, i16, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
