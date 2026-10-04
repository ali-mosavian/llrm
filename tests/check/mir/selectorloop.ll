; RUN: llrm-mir %s
; CHECK: define {{.*}} @_f(

target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32"

define i16 @_f(ptr addrspace(1) %p0, ptr addrspace(1) %p1, ptr addrspace(1) %p2, ptr addrspace(1) %p3, ptr addrspace(1) %p4, ptr addrspace(1) %p5, i16 %n) addrspace(1) {
entry:
  br label %loop

loop:
  %i = phi i16 [ 0, %entry ], [ %i2, %loop ]
  %s = phi i16 [ 0, %entry ], [ %s5, %loop ]
  %g0 = getelementptr i16, ptr addrspace(1) %p0, i16 %i
  %v0 = load i16, ptr addrspace(1) %g0, !tbaa !9
  %s0 = add i16 %s, %v0
  %g1 = getelementptr i16, ptr addrspace(1) %p1, i16 %i
  %v1 = load i16, ptr addrspace(1) %g1, !tbaa !9
  %s1 = add i16 %s0, %v1
  %g2 = getelementptr i16, ptr addrspace(1) %p2, i16 %i
  %v2 = load i16, ptr addrspace(1) %g2, !tbaa !9
  %s2 = add i16 %s1, %v2
  %g3 = getelementptr i16, ptr addrspace(1) %p3, i16 %i
  %v3 = load i16, ptr addrspace(1) %g3, !tbaa !9
  %s3 = add i16 %s2, %v3
  %g4 = getelementptr i16, ptr addrspace(1) %p4, i16 %i
  %v4 = load i16, ptr addrspace(1) %g4, !tbaa !9
  %s4 = add i16 %s3, %v4
  %g5 = getelementptr i16, ptr addrspace(1) %p5, i16 %i
  %v5 = load i16, ptr addrspace(1) %g5, !tbaa !9
  %s5 = add i16 %s4, %v5
  %i2 = add i16 %i, 1
  %e = icmp ne i16 %i2, %n
  br i1 %e, label %loop, label %done

done:
  ret i16 %s5
}

!0 = !{!"llrm hir"}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!"omnipotent char", !5, i64 0}
!8 = !{!"int2", !6, i64 0}
!9 = !{!8, !8, i64 0}
