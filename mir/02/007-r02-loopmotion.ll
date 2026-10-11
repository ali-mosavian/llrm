@_vx = internal global [202 x i8] zeroinitializer
@_vy = internal global [202 x i8] zeroinitializer
@_vz = internal global [202 x i8] zeroinitializer
@_px = internal global [202 x i8] zeroinitializer
@_py = internal global [202 x i8] zeroinitializer
@_pz = internal global [202 x i8] zeroinitializer

declare internal watcallcc void @advance_(i16) memory(readwrite, argmem: none, inaccessiblemem: none) norecurse

declare watcallcc i32 @bench_particle_() memory(readwrite, argmem: none, inaccessiblemem: none) norecurse

declare i32 @_main() memory(readwrite, argmem: none)

declare watcallcc i32 @report_(i32)

define internal watcallcc i32 @pipeline.body() memory(readwrite, argmem: none, inaccessiblemem: none) norecurse {
b1:
  %0 = alloca i32
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i32
  store i16 0, ptr %1, !tbaa !9
  br label %b2

b2:
  %5 = phi i16 [ 0, %b1 ], [ %23, %b4 ]
  %6 = sext i16 %5 to i32
  %7 = icmp sle i16 %5, 100
  br i1 %7, label %b4, label %b3

b3:
  store i16 %5, ptr %1, !tbaa !9
  store i16 1, ptr %2, !tbaa !9
  br label %b5

b4:
  %8 = sub i16 %5, 50
  store i16 %8, ptr %3, !tbaa !9
  %9 = mul i32 %6, 2
  %10 = getelementptr inbounds i8, ptr @_px, i32 %9
  store i16 %8, ptr %10, !tbaa !9
  %11 = sext i16 %8 to i32
  %12 = mul i16 2, %8
  %13 = getelementptr inbounds i8, ptr @_py, i32 %9
  store i16 %12, ptr %13, !tbaa !9
  %14 = sub i16 0, %8
  %15 = getelementptr inbounds i8, ptr @_pz, i32 %9
  store i16 %14, ptr %15, !tbaa !9
  %16 = and i16 %8, 7
  %17 = getelementptr inbounds i8, ptr @_vx, i32 %9
  store i16 %16, ptr %17, !tbaa !9
  %18 = and i32 %11, 3
  %19 = sub nsw i32 3, %18
  %20 = trunc i32 %19 to i16
  %21 = getelementptr inbounds i8, ptr @_vy, i32 %9
  store i16 %20, ptr %21, !tbaa !9
  %22 = getelementptr inbounds i8, ptr @_vz, i32 %9
  store i16 1, ptr %22, !tbaa !9
  %23 = add i16 %5, 1
  br label %b2

b5:
  %24 = phi i16 [ 1, %b3 ], [ %26, %b7 ]
  %25 = icmp sle i16 %24, 100
  br i1 %25, label %b7, label %b6

b6:
  store i32 0, ptr %4, !tbaa !11
  store i16 0, ptr %1, !tbaa !9
  br label %b8

b7:
  call watcallcc void @advance_(i16 50)
  %26 = add i16 %24, 1
  store i16 %26, ptr %2, !tbaa !9
  br label %b5

b8:
  %27 = phi i32 [ 0, %b6 ], [ %46, %b10 ]
  %28 = phi i16 [ 0, %b6 ], [ %47, %b10 ]
  %29 = sext i16 %28 to i32
  %30 = icmp sle i16 %28, 100
  br i1 %30, label %b10, label %b9

b9:
  %31 = phi i32 [ %27, %b8 ]
  store i32 %27, ptr %4, !tbaa !11
  store i16 %28, ptr %1, !tbaa !9
  store i32 %31, ptr %0, !tbaa !11
  ret i32 %31

b10:
  %32 = mul i32 %29, 2
  %33 = getelementptr inbounds i8, ptr @_px, i32 %32
  %34 = load i16, ptr %33, !tbaa !9
  %35 = sext i16 %34 to i32
  %36 = add nsw i32 %27, %35
  %37 = getelementptr inbounds i8, ptr @_py, i32 %32
  %38 = load i16, ptr %37, !tbaa !9
  %39 = sext i16 %38 to i32
  %40 = mul nsw i32 3, %39
  %41 = add nsw i32 %36, %40
  %42 = getelementptr inbounds i8, ptr @_pz, i32 %32
  %43 = load i16, ptr %42, !tbaa !9
  %44 = sext i16 %43 to i32
  %45 = mul nsw i32 7, %44
  %46 = add nsw i32 %41, %45
  %47 = add i16 %28, 1
  br label %b8
}

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
