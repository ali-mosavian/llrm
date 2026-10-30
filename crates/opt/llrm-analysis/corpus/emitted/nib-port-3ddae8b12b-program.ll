target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$f32_40400000 = internal constant [4 x i8] c"\00\00@@"
@$f32_40000000 = internal constant [4 x i8] c"\00\00\00@"

define internal cc 1005 float @value() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = load float, ptr @$f32_40400000, !tbaa !2
  %1 = load float, ptr @$f32_40000000, !tbaa !2
  %2 = fdiv float %0, %1
  ret float %2
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
