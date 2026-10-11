target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

@_vx = internal global [202 x i8] zeroinitializer
@_vy = internal global [202 x i8] zeroinitializer
@_vz = internal global [202 x i8] zeroinitializer
@_px = internal global [202 x i8] zeroinitializer
@_py = internal global [202 x i8] zeroinitializer
@_pz = internal global [202 x i8] zeroinitializer

define internal watcallcc void @advance_(i16 %0) memory(readwrite, argmem: none, inaccessiblemem: none) norecurse {
b1:
  %1 = sext i16 %0 to i32
  %2 = sub i16 50, %0
  %3 = add nsw i32 50, %1
  br label %b2

b2:
  %4 = phi i16 [ %2, %b1 ], [ %23, %b4 ]
  %5 = sext i16 %4 to i32
  %6 = icmp sle i32 %5, %3
  br i1 %6, label %b4, label %b3

b3:
  ret void

b4:
  %7 = mul i32 %5, 2
  %8 = getelementptr inbounds i8, ptr @_px, i32 %7
  %9 = load i16, ptr %8, !tbaa !9
  %10 = getelementptr inbounds i8, ptr @_vx, i32 %7
  %11 = load i16, ptr %10, !tbaa !9
  %12 = add i16 %9, %11
  store i16 %12, ptr %8, !tbaa !9
  %13 = getelementptr inbounds i8, ptr @_py, i32 %7
  %14 = load i16, ptr %13, !tbaa !9
  %15 = getelementptr inbounds i8, ptr @_vy, i32 %7
  %16 = load i16, ptr %15, !tbaa !9
  %17 = add i16 %14, %16
  store i16 %17, ptr %13, !tbaa !9
  %18 = getelementptr inbounds i8, ptr @_pz, i32 %7
  %19 = load i16, ptr %18, !tbaa !9
  %20 = getelementptr inbounds i8, ptr @_vz, i32 %7
  %21 = load i16, ptr %20, !tbaa !9
  %22 = add i16 %19, %21
  store i16 %22, ptr %18, !tbaa !9
  %23 = add i16 %4, 1
  br label %b2
}

define watcallcc i32 @bench_particle_() memory(readwrite, argmem: none, inaccessiblemem: none) norecurse {
b1:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b1 ], [ %18, %b4 ]
  %1 = sext i16 %0 to i32
  %2 = icmp sle i16 %0, 100
  br i1 %2, label %b4, label %b3

b3:
  br label %b5

b4:
  %3 = sub i16 %0, 50
  %4 = mul i32 %1, 2
  %5 = getelementptr inbounds i8, ptr @_px, i32 %4
  store i16 %3, ptr %5, !tbaa !9
  %6 = sext i16 %3 to i32
  %7 = mul i16 2, %3
  %8 = getelementptr inbounds i8, ptr @_py, i32 %4
  store i16 %7, ptr %8, !tbaa !9
  %9 = sub i16 0, %3
  %10 = getelementptr inbounds i8, ptr @_pz, i32 %4
  store i16 %9, ptr %10, !tbaa !9
  %11 = and i16 %3, 7
  %12 = getelementptr inbounds i8, ptr @_vx, i32 %4
  store i16 %11, ptr %12, !tbaa !9
  %13 = and i32 %6, 3
  %14 = sub nsw i32 3, %13
  %15 = trunc i32 %14 to i16
  %16 = getelementptr inbounds i8, ptr @_vy, i32 %4
  store i16 %15, ptr %16, !tbaa !9
  %17 = getelementptr inbounds i8, ptr @_vz, i32 %4
  store i16 1, ptr %17, !tbaa !9
  %18 = add i16 %0, 1
  br label %b2

b5:
  %19 = phi i16 [ 1, %b3 ], [ %21, %b7 ]
  %20 = icmp sle i16 %19, 100
  br i1 %20, label %b7, label %b6

b6:
  br label %b8

b7:
  call watcallcc void @advance_(i16 50)
  %21 = add i16 %19, 1
  br label %b5

b8:
  %22 = phi i32 [ 0, %b6 ], [ %41, %b10 ]
  %23 = phi i16 [ 0, %b6 ], [ %42, %b10 ]
  %24 = sext i16 %23 to i32
  %25 = icmp sle i16 %23, 100
  br i1 %25, label %b10, label %b9

b9:
  %26 = phi i32 [ %22, %b8 ]
  ret i32 %26

b10:
  %27 = mul i32 %24, 2
  %28 = getelementptr inbounds i8, ptr @_px, i32 %27
  %29 = load i16, ptr %28, !tbaa !9
  %30 = sext i16 %29 to i32
  %31 = add nsw i32 %22, %30
  %32 = getelementptr inbounds i8, ptr @_py, i32 %27
  %33 = load i16, ptr %32, !tbaa !9
  %34 = sext i16 %33 to i32
  %35 = mul nsw i32 3, %34
  %36 = add nsw i32 %31, %35
  %37 = getelementptr inbounds i8, ptr @_pz, i32 %27
  %38 = load i16, ptr %37, !tbaa !9
  %39 = sext i16 %38 to i32
  %40 = mul nsw i32 7, %39
  %41 = add nsw i32 %36, %40
  %42 = add i16 %23, 1
  br label %b8
}

define i32 @_main() memory(readwrite, argmem: none) {
b1:
  %0 = call watcallcc i32 @bench_particle_()
  %1 = call watcallcc i32 @report_(i32 %0)
  ret i32 0
}

declare watcallcc i32 @report_(i32)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!"omnipotent char", !5, i64 0}
!7 = !{!6, !6, i64 0}
!8 = !{!"int2", !6, i64 0}
!9 = !{!8, !8, i64 0}
!10 = !{!"int4", !6, i64 0}
!11 = !{!10, !10, i64 0}
!12 = !{!"int8", !6, i64 0}
!13 = !{!12, !12, i64 0}
!14 = !{!"float4", !6, i64 0}
!15 = !{!14, !14, i64 0}
!16 = !{!"float8", !6, i64 0}
!17 = !{!16, !16, i64 0}
!18 = !{!"float10", !6, i64 0}
!19 = !{!18, !18, i64 0}
!20 = !{!"pointer2", !6, i64 0}
!21 = !{!20, !20, i64 0}
!22 = !{!"pointer4", !6, i64 0}
!23 = !{!22, !22, i64 0}
