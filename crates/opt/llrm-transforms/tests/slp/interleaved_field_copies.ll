target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

; save(A *x, A *y) { y->a = x->a; y->b = x->b; y->c = x->c; y->d = x->d; }
; Each load stands after the store before it; the fields are distinct TBAA
; paths, so no store can change what a later load reads.
define void @save(ptr %x, ptr %y) {
  %a = load i64, ptr %x, align 8, !tbaa !4
  store i64 %a, ptr %y, align 8, !tbaa !4
  %x1 = getelementptr inbounds i8, ptr %x, i64 8
  %b = load i64, ptr %x1, align 8, !tbaa !5
  %y1 = getelementptr inbounds i8, ptr %y, i64 8
  store i64 %b, ptr %y1, align 8, !tbaa !5
  %x2 = getelementptr inbounds i8, ptr %x, i64 16
  %c = load i64, ptr %x2, align 8, !tbaa !6
  %y2 = getelementptr inbounds i8, ptr %y, i64 16
  store i64 %c, ptr %y2, align 8, !tbaa !6
  %x3 = getelementptr inbounds i8, ptr %x, i64 24
  %d = load i64, ptr %x3, align 8, !tbaa !7
  %y3 = getelementptr inbounds i8, ptr %y, i64 24
  store i64 %d, ptr %y3, align 8, !tbaa !7
  ret void
}

; the same copy with one TBAA tag for every field: a store may be what a later load reads
define void @save_same_tag(ptr %x, ptr %y) {
  %a = load i64, ptr %x, align 8, !tbaa !3
  store i64 %a, ptr %y, align 8, !tbaa !3
  %x1 = getelementptr inbounds i8, ptr %x, i64 8
  %b = load i64, ptr %x1, align 8, !tbaa !3
  %y1 = getelementptr inbounds i8, ptr %y, i64 8
  store i64 %b, ptr %y1, align 8, !tbaa !3
  ret void
}

!0 = !{!"Simple C/C++ TBAA"}
!1 = !{!"omnipotent char", !0, i64 0}
!3 = !{!"long", !1, i64 0}
!8 = !{!"A", !3, i64 0, !3, i64 8, !3, i64 16, !3, i64 24}
!4 = !{!8, !3, i64 0}
!5 = !{!8, !3, i64 8}
!6 = !{!8, !3, i64 16}
!7 = !{!8, !3, i64 24}
