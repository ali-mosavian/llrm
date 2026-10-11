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
