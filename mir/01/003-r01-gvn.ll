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
  %5 = sub nsw i32 50, %4
  %6 = trunc i32 %5 to i16
  store i16 %6, ptr %3, !tbaa !9
  %7 = add nsw i32 50, %4
  br label %b2

b2:
  %8 = load i16, ptr %3, !tbaa !9
  %9 = sext i16 %8 to i32
  %10 = icmp sle i32 %9, %7
  %11 = zext i1 %10 to i32
  br i1 %10, label %b4, label %b3

b3:
  ret void

b4:
  %12 = mul i32 %9, 2
  %13 = getelementptr inbounds i8, ptr @_px, i32 %12
  %14 = load i16, ptr %13, !tbaa !9
  %15 = getelementptr inbounds i8, ptr @_vx, i32 %12
  %16 = load i16, ptr %15, !tbaa !9
  %17 = sext i16 %14 to i32
  %18 = sext i16 %16 to i32
  %19 = add nsw i32 %17, %18
  %20 = trunc i32 %19 to i16
  store i16 %20, ptr %13, !tbaa !9
  %21 = getelementptr inbounds i8, ptr @_py, i32 %12
  %22 = load i16, ptr %21, !tbaa !9
  %23 = getelementptr inbounds i8, ptr @_vy, i32 %12
  %24 = load i16, ptr %23, !tbaa !9
  %25 = sext i16 %22 to i32
  %26 = sext i16 %24 to i32
  %27 = add nsw i32 %25, %26
  %28 = trunc i32 %27 to i16
  store i16 %28, ptr %21, !tbaa !9
  %29 = getelementptr inbounds i8, ptr @_pz, i32 %12
  %30 = load i16, ptr %29, !tbaa !9
  %31 = getelementptr inbounds i8, ptr @_vz, i32 %12
  %32 = load i16, ptr %31, !tbaa !9
  %33 = sext i16 %30 to i32
  %34 = sext i16 %32 to i32
  %35 = add nsw i32 %33, %34
  %36 = trunc i32 %35 to i16
  store i16 %36, ptr %29, !tbaa !9
  %37 = add nsw i32 %9, 1
  %38 = trunc i32 %37 to i16
  store i16 %38, ptr %3, !tbaa !9
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
