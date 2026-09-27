target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @describe() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca [6 x i8]
  store i16 0, ptr %0
  store i16 0, ptr %1
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 6, i1 false)
  store i16 3, ptr %0, !tbaa !2
  store i16 3, ptr %1, !tbaa !2
  %3 = sub i16 0, 0
  %4 = getelementptr inbounds i16, ptr %2, i16 %3
  store i16 10, ptr %4, !tbaa !2
  %5 = sub i16 1, 0
  %6 = getelementptr inbounds i16, ptr %2, i16 %5
  store i16 20, ptr %6, !tbaa !2
  %7 = sub i16 2, 0
  %8 = getelementptr inbounds i16, ptr %2, i16 %7
  store i16 30, ptr %8, !tbaa !2
  %9 = add i16 3, 3
  %10 = add i16 %9, 3
  ret i16 %10
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
