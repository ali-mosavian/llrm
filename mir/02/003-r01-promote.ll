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
  %5 = trunc i32 0 to i16
  store i16 0, ptr %1, !tbaa !9
  br label %b2

b2:
  %6 = phi i16 [ 0, %b1 ], [ %32, %b4 ]
  %7 = sext i16 %6 to i32
  %8 = icmp sle i32 %7, 100
  %9 = zext i1 %8 to i32
  br i1 %8, label %b4, label %b3

b3:
  %10 = trunc i32 1 to i16
  store i16 1, ptr %2, !tbaa !9
  br label %b5

b4:
  %11 = sub nsw i32 %7, 50
  %12 = trunc i32 %11 to i16
  store i16 %12, ptr %3, !tbaa !9
  %13 = mul i32 %7, 2
  %14 = getelementptr inbounds i8, ptr @_px, i32 %13
  store i16 %12, ptr %14, !tbaa !9
  %15 = sext i16 %12 to i32
  %16 = mul nsw i32 2, %15
  %17 = trunc i32 %16 to i16
  %18 = getelementptr inbounds i8, ptr @_py, i32 %13
  store i16 %17, ptr %18, !tbaa !9
  %19 = sub nsw i32 0, %15
  %20 = trunc i32 %19 to i16
  %21 = getelementptr inbounds i8, ptr @_pz, i32 %13
  store i16 %20, ptr %21, !tbaa !9
  %22 = and i32 %15, 7
  %23 = trunc i32 %22 to i16
  %24 = getelementptr inbounds i8, ptr @_vx, i32 %13
  store i16 %23, ptr %24, !tbaa !9
  %25 = and i32 %15, 3
  %26 = sub nsw i32 3, %25
  %27 = trunc i32 %26 to i16
  %28 = getelementptr inbounds i8, ptr @_vy, i32 %13
  store i16 %27, ptr %28, !tbaa !9
  %29 = trunc i32 1 to i16
  %30 = getelementptr inbounds i8, ptr @_vz, i32 %13
  store i16 1, ptr %30, !tbaa !9
  %31 = add nsw i32 %7, 1
  %32 = trunc i32 %31 to i16
  store i16 %32, ptr %1, !tbaa !9
  br label %b2

b5:
  %33 = phi i16 [ 1, %b3 ], [ %38, %b7 ]
  %34 = sext i16 %33 to i32
  %35 = icmp sle i32 %34, 100
  %36 = zext i1 %35 to i32
  br i1 %35, label %b7, label %b6

b6:
  store i32 0, ptr %4, !tbaa !11
  store i16 0, ptr %1, !tbaa !9
  br label %b8

b7:
  call watcallcc void @advance_(i16 50)
  %37 = add nsw i32 %34, 1
  %38 = trunc i32 %37 to i16
  store i16 %38, ptr %2, !tbaa !9
  br label %b5

b8:
  %39 = phi i32 [ 0, %b6 ], [ %58, %b10 ]
  %40 = phi i16 [ 0, %b6 ], [ %60, %b10 ]
  %41 = sext i16 %40 to i32
  %42 = icmp sle i32 %41, 100
  %43 = zext i1 %42 to i32
  br i1 %42, label %b10, label %b9

b9:
  store i32 %39, ptr %0, !tbaa !11
  ret i32 %39

b10:
  %44 = mul i32 %41, 2
  %45 = getelementptr inbounds i8, ptr @_px, i32 %44
  %46 = load i16, ptr %45, !tbaa !9
  %47 = sext i16 %46 to i32
  %48 = add nsw i32 %39, %47
  %49 = getelementptr inbounds i8, ptr @_py, i32 %44
  %50 = load i16, ptr %49, !tbaa !9
  %51 = sext i16 %50 to i32
  %52 = mul nsw i32 3, %51
  %53 = add nsw i32 %48, %52
  %54 = getelementptr inbounds i8, ptr @_pz, i32 %44
  %55 = load i16, ptr %54, !tbaa !9
  %56 = sext i16 %55 to i32
  %57 = mul nsw i32 7, %56
  %58 = add nsw i32 %53, %57
  store i32 %58, ptr %4, !tbaa !11
  %59 = add nsw i32 %41, 1
  %60 = trunc i32 %59 to i16
  store i16 %60, ptr %1, !tbaa !9
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
