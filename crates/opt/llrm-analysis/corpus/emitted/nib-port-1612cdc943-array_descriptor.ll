target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i16 @describe() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca [6 x i8]
  store i16 3, ptr %0, !tbaa !2
  store i16 3, ptr %1, !tbaa !2
  %3 = getelementptr inbounds i16, ptr %2, i16 0
  store i16 10, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i16, ptr %2, i16 1
  store i16 20, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i16, ptr %2, i16 2
  store i16 30, ptr %5, !tbaa !2
  ret i16 9
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
