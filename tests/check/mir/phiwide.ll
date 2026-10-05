; RUN: llrm-mir %s
; CHECK: define {{.*}} @_f(

target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32"

@T = global [4096 x i16] zeroinitializer

define i16 @_f(i16 %p0, i16 %p1, i16 %p2, i16 %p3, i16 %p4, i16 %p5, i16 %p6, i16 %p7, i16 %p8, i16 %p9, i16 %p10, i16 %p11, i16 %p12, i16 %p13, i16 %p14, i16 %p15, i16 %n) addrspace(1) {
entry:
  br label %loop

loop:
  %i = phi i16 [ 0, %entry ], [ %i2, %loop ]
  %a0 = phi i16 [ %p0, %entry ], [ %b0, %loop ]
  %a1 = phi i16 [ %p1, %entry ], [ %b1, %loop ]
  %a2 = phi i16 [ %p2, %entry ], [ %b2, %loop ]
  %a3 = phi i16 [ %p3, %entry ], [ %b3, %loop ]
  %a4 = phi i16 [ %p4, %entry ], [ %b4, %loop ]
  %a5 = phi i16 [ %p5, %entry ], [ %b5, %loop ]
  %a6 = phi i16 [ %p6, %entry ], [ %b6, %loop ]
  %a7 = phi i16 [ %p7, %entry ], [ %b7, %loop ]
  %a8 = phi i16 [ %p8, %entry ], [ %b8, %loop ]
  %a9 = phi i16 [ %p9, %entry ], [ %b9, %loop ]
  %a10 = phi i16 [ %p10, %entry ], [ %b10, %loop ]
  %a11 = phi i16 [ %p11, %entry ], [ %b11, %loop ]
  %a12 = phi i16 [ %p12, %entry ], [ %b12, %loop ]
  %a13 = phi i16 [ %p13, %entry ], [ %b13, %loop ]
  %a14 = phi i16 [ %p14, %entry ], [ %b14, %loop ]
  %a15 = phi i16 [ %p15, %entry ], [ %b15, %loop ]
  %tp = getelementptr i16, ptr @T, i16 %i
  %t = load i16, ptr %tp, !tbaa !9
  %b0 = add i16 %a1, %t
  %b1 = add i16 %a2, %t
  %b2 = add i16 %a3, %t
  %b3 = add i16 %a4, %t
  %b4 = add i16 %a5, %t
  %b5 = add i16 %a6, %t
  %b6 = add i16 %a7, %t
  %b7 = add i16 %a8, %t
  %b8 = add i16 %a9, %t
  %b9 = add i16 %a10, %t
  %b10 = add i16 %a11, %t
  %b11 = add i16 %a12, %t
  %b12 = add i16 %a13, %t
  %b13 = add i16 %a14, %t
  %b14 = add i16 %a15, %t
  %b15 = add i16 %a0, %t
  %i2 = add i16 %i, 1
  %e = icmp ne i16 %i2, %n
  br i1 %e, label %loop, label %done

done:
  %s1 = add i16 %a0, %a1
  %s2 = add i16 %s1, %a2
  %s3 = add i16 %s2, %a3
  %s4 = add i16 %s3, %a4
  %s5 = add i16 %s4, %a5
  %s6 = add i16 %s5, %a6
  %s7 = add i16 %s6, %a7
  %s8 = add i16 %s7, %a8
  %s9 = add i16 %s8, %a9
  %s10 = add i16 %s9, %a10
  %s11 = add i16 %s10, %a11
  %s12 = add i16 %s11, %a12
  %s13 = add i16 %s12, %a13
  %s14 = add i16 %s13, %a14
  %s15 = add i16 %s14, %a15
  ret i16 %s15
}

!0 = !{!"llrm hir"}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!"omnipotent char", !5, i64 0}
!8 = !{!"int2", !6, i64 0}
!9 = !{!8, !8, i64 0}
