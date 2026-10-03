; RUN: llrm-mir %s
; CHECK: define {{.*}} @_f(

target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32"

@Y = global [8 x i16] zeroinitializer
@T = global [4096 x i16] zeroinitializer
@SP = global [4096 x i16] zeroinitializer
@OUT = global [4096 x i8] zeroinitializer

define void @_f(i16 %p0, i16 %p1, i16 %p2, i16 %p3, i16 %p4, i16 %p5, i16 %p6, i32 %n) addrspace(1) {
b1:
  %s0 = shl i16 %p0, 1
  %o0 = sub i16 0, %s0
  %yp0 = getelementptr i16, ptr @Y, i16 0
  %y0 = load i16, ptr %yp0, !tbaa !9
  %s1 = shl i16 %p1, 1
  %o1 = sub i16 0, %s1
  %yp1 = getelementptr i16, ptr @Y, i16 1
  %y1 = load i16, ptr %yp1, !tbaa !9
  %s2 = shl i16 %p2, 1
  %o2 = sub i16 0, %s2
  %yp2 = getelementptr i16, ptr @Y, i16 2
  %y2 = load i16, ptr %yp2, !tbaa !9
  %s3 = shl i16 %p3, 1
  %o3 = sub i16 0, %s3
  %yp3 = getelementptr i16, ptr @Y, i16 3
  %y3 = load i16, ptr %yp3, !tbaa !9
  %s4 = shl i16 %p4, 1
  %o4 = sub i16 0, %s4
  %yp4 = getelementptr i16, ptr @Y, i16 4
  %y4 = load i16, ptr %yp4, !tbaa !9
  %s5 = shl i16 %p5, 1
  %o5 = sub i16 0, %s5
  %yp5 = getelementptr i16, ptr @Y, i16 5
  %y5 = load i16, ptr %yp5, !tbaa !9
  %s6 = shl i16 %p6, 1
  %o6 = sub i16 0, %s6
  %yp6 = getelementptr i16, ptr @Y, i16 6
  %y6 = load i16, ptr %yp6, !tbaa !9
  br label %b2

b2:
  %i = phi i32 [ -272, %b1 ], [ %i.next, %b2 ]
  %ii = trunc i32 %i to i16
  %x = shl i16 %ii, 1
  %ix0 = add i16 %x, %o0
  %tp0 = getelementptr i16, ptr @T, i16 %ix0
  %t0 = load i16, ptr %tp0, !tbaa !11
  %a0 = add i16 %t0, %y0
  %sh0 = shl i16 %a0, 1
  %sp0 = getelementptr i16, ptr @SP, i16 %sh0
  %v0 = load i16, ptr %sp0, !tbaa !13
  %c0 = add i16 %v0, 0
  %ix1 = add i16 %x, %o1
  %tp1 = getelementptr i16, ptr @T, i16 %ix1
  %t1 = load i16, ptr %tp1, !tbaa !11
  %a1 = add i16 %t1, %y1
  %sh1 = shl i16 %a1, 1
  %sp1 = getelementptr i16, ptr @SP, i16 %sh1
  %v1 = load i16, ptr %sp1, !tbaa !13
  %c1 = add i16 %c0, %v1
  %ix2 = add i16 %x, %o2
  %tp2 = getelementptr i16, ptr @T, i16 %ix2
  %t2 = load i16, ptr %tp2, !tbaa !11
  %a2 = add i16 %t2, %y2
  %sh2 = shl i16 %a2, 1
  %sp2 = getelementptr i16, ptr @SP, i16 %sh2
  %v2 = load i16, ptr %sp2, !tbaa !13
  %c2 = add i16 %c1, %v2
  %ix3 = add i16 %x, %o3
  %tp3 = getelementptr i16, ptr @T, i16 %ix3
  %t3 = load i16, ptr %tp3, !tbaa !11
  %a3 = add i16 %t3, %y3
  %sh3 = shl i16 %a3, 1
  %sp3 = getelementptr i16, ptr @SP, i16 %sh3
  %v3 = load i16, ptr %sp3, !tbaa !13
  %c3 = add i16 %c2, %v3
  %ix4 = add i16 %x, %o4
  %tp4 = getelementptr i16, ptr @T, i16 %ix4
  %t4 = load i16, ptr %tp4, !tbaa !11
  %a4 = add i16 %t4, %y4
  %sh4 = shl i16 %a4, 1
  %sp4 = getelementptr i16, ptr @SP, i16 %sh4
  %v4 = load i16, ptr %sp4, !tbaa !13
  %c4 = add i16 %c3, %v4
  %ix5 = add i16 %x, %o5
  %tp5 = getelementptr i16, ptr @T, i16 %ix5
  %t5 = load i16, ptr %tp5, !tbaa !11
  %a5 = add i16 %t5, %y5
  %sh5 = shl i16 %a5, 1
  %sp5 = getelementptr i16, ptr @SP, i16 %sh5
  %v5 = load i16, ptr %sp5, !tbaa !13
  %c5 = add i16 %c4, %v5
  %ix6 = add i16 %x, %o6
  %tp6 = getelementptr i16, ptr @T, i16 %ix6
  %t6 = load i16, ptr %tp6, !tbaa !11
  %a6 = add i16 %t6, %y6
  %sh6 = shl i16 %a6, 1
  %sp6 = getelementptr i16, ptr @SP, i16 %sh6
  %v6 = load i16, ptr %sp6, !tbaa !13
  %c6 = add i16 %c5, %v6
  %outp = getelementptr i8, ptr @OUT, i16 %ii
  %lo = trunc i16 %c6 to i8
  store i8 %lo, ptr %outp, !tbaa !15
  %i.next = add i32 %i, 1
  %cnd = icmp ne i32 %i.next, 0
  br i1 %cnd, label %b2, label %b3

b3:
  ret void
}

!0 = !{!"llrm hir"}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!"omnipotent char", !5, i64 0}
!8 = !{!"int2", !6, i64 0}
!9 = !{!8, !8, i64 0}
!10 = !{!"t2", !6, i64 0}
!11 = !{!10, !10, i64 0}
!12 = !{!"sp2", !6, i64 0}
!13 = !{!12, !12, i64 0}
!14 = !{!"out1", !6, i64 0}
!15 = !{!14, !14, i64 0}
