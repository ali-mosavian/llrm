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

define internal watcallcc void @pipeline.body(i16 %0) memory(readwrite, argmem: none, inaccessiblemem: none) norecurse {
b1:
  %1 = alloca i16
  %2 = alloca i32
  %3 = alloca i16
  store i16 %0, ptr %1, !tbaa !9
  %4 = sext i16 %0 to i32
  %5 = sub i16 50, %0
  store i16 %5, ptr %3, !tbaa !9
  %6 = add nsw i32 50, %4
  br label %b2

b2:
  %7 = phi i16 [ %5, %b1 ], [ %27, %b4 ]
  %8 = sext i16 %7 to i32
  %9 = icmp sle i32 %8, %6
  %10 = zext i1 %9 to i32
  br i1 %9, label %b4, label %b3

b3:
  ret void

b4:
  %11 = mul i32 %8, 2
  %12 = getelementptr inbounds i8, ptr @_px, i32 %11
  %13 = load i16, ptr %12, !tbaa !9
  %14 = getelementptr inbounds i8, ptr @_vx, i32 %11
  %15 = load i16, ptr %14, !tbaa !9
  %16 = add i16 %13, %15
  store i16 %16, ptr %12, !tbaa !9
  %17 = getelementptr inbounds i8, ptr @_py, i32 %11
  %18 = load i16, ptr %17, !tbaa !9
  %19 = getelementptr inbounds i8, ptr @_vy, i32 %11
  %20 = load i16, ptr %19, !tbaa !9
  %21 = add i16 %18, %20
  store i16 %21, ptr %17, !tbaa !9
  %22 = getelementptr inbounds i8, ptr @_pz, i32 %11
  %23 = load i16, ptr %22, !tbaa !9
  %24 = getelementptr inbounds i8, ptr @_vz, i32 %11
  %25 = load i16, ptr %24, !tbaa !9
  %26 = add i16 %23, %25
  store i16 %26, ptr %22, !tbaa !9
  %27 = add i16 %7, 1
  store i16 %27, ptr %3, !tbaa !9
  br label %b2
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
