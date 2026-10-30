target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i16 @value() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i8
  %1 = alloca i8
  store i8 1, ptr %1, !tbaa !2
  br i1 true, label %b3, label %b2

b2:
  store i8 0, ptr %0, !tbaa !2
  br i1 false, label %b4, label %b5

b3:
  %2 = load i8, ptr %1, !tbaa !2, !range !5
  %3 = zext i8 %2 to i16
  %4 = and i16 %3, 1
  ret i16 %4

b4:
  store i8 0, ptr %0, !tbaa !2
  br label %b5

b5:
  %5 = load i8, ptr %0, !tbaa !2, !range !6
  store i8 %5, ptr %1, !tbaa !2
  br label %b3
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
