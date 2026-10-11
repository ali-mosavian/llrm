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
  %6 = phi i16 [ 0, %b1 ], [ %27, %b4 ]
  %7 = sext i16 %6 to i32
  %8 = icmp sle i16 %6, 100
  %9 = zext i1 %8 to i32
  br i1 %8, label %b4, label %b3

b3:
  %10 = trunc i32 1 to i16
  store i16 1, ptr %2, !tbaa !9
  br label %b5

b4:
  %11 = sub i16 %6, 50
  store i16 %11, ptr %3, !tbaa !9
  %12 = mul i32 %7, 2
  %13 = getelementptr inbounds i8, ptr @_px, i32 %12
  store i16 %11, ptr %13, !tbaa !9
  %14 = sext i16 %11 to i32
  %15 = mul i16 2, %11
  %16 = getelementptr inbounds i8, ptr @_py, i32 %12
  store i16 %15, ptr %16, !tbaa !9
  %17 = sub i16 0, %11
  %18 = getelementptr inbounds i8, ptr @_pz, i32 %12
  store i16 %17, ptr %18, !tbaa !9
  %19 = and i16 %11, 7
  %20 = getelementptr inbounds i8, ptr @_vx, i32 %12
  store i16 %19, ptr %20, !tbaa !9
  %21 = and i32 %14, 3
  %22 = sub nsw i32 3, %21
  %23 = trunc i32 %22 to i16
  %24 = getelementptr inbounds i8, ptr @_vy, i32 %12
  store i16 %23, ptr %24, !tbaa !9
  %25 = trunc i32 1 to i16
  %26 = getelementptr inbounds i8, ptr @_vz, i32 %12
  store i16 1, ptr %26, !tbaa !9
  %27 = add i16 %6, 1
  store i16 %27, ptr %1, !tbaa !9
  br label %b2

b5:
  %28 = phi i16 [ 1, %b3 ], [ %31, %b7 ]
  %29 = icmp sle i16 %28, 100
  %30 = zext i1 %29 to i32
  br i1 %29, label %b7, label %b6

b6:
  store i32 0, ptr %4, !tbaa !11
  store i16 0, ptr %1, !tbaa !9
  br label %b8

b7:
  call watcallcc void @advance_(i16 50)
  %31 = add i16 %28, 1
  store i16 %31, ptr %2, !tbaa !9
  br label %b5

b8:
  %32 = phi i32 [ 0, %b6 ], [ %51, %b10 ]
  %33 = phi i16 [ 0, %b6 ], [ %52, %b10 ]
  %34 = sext i16 %33 to i32
  %35 = icmp sle i16 %33, 100
  %36 = zext i1 %35 to i32
  br i1 %35, label %b10, label %b9

b9:
  store i32 %32, ptr %0, !tbaa !11
  ret i32 %32

b10:
  %37 = mul i32 %34, 2
  %38 = getelementptr inbounds i8, ptr @_px, i32 %37
  %39 = load i16, ptr %38, !tbaa !9
  %40 = sext i16 %39 to i32
  %41 = add nsw i32 %32, %40
  %42 = getelementptr inbounds i8, ptr @_py, i32 %37
  %43 = load i16, ptr %42, !tbaa !9
  %44 = sext i16 %43 to i32
  %45 = mul nsw i32 3, %44
  %46 = add nsw i32 %41, %45
  %47 = getelementptr inbounds i8, ptr @_pz, i32 %37
  %48 = load i16, ptr %47, !tbaa !9
  %49 = sext i16 %48 to i32
  %50 = mul nsw i32 7, %49
  %51 = add nsw i32 %46, %50
  store i32 %51, ptr %4, !tbaa !11
  %52 = add i16 %33, 1
  store i16 %52, ptr %1, !tbaa !9
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
