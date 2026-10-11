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
  %19 = phi i16 [ 1, %b3 ], [ %30, %29 ]
  %20 = icmp sle i16 %19, 100
  br i1 %20, label %21, label %b6

b6:
  br label %b8

21:
  %22 = sext i16 50 to i32
  %23 = sub i16 50, 50
  %24 = add nsw i32 50, 50
  br label %25

25:
  %26 = phi i16 [ 0, %21 ], [ %48, %31 ]
  %27 = sext i16 %26 to i32
  %28 = icmp sle i32 %27, 100
  br i1 %28, label %31, label %29

29:
  %30 = add i16 %19, 1
  br label %b5

31:
  %32 = mul i32 %27, 2
  %33 = getelementptr inbounds i8, ptr @_px, i32 %32
  %34 = load i16, ptr %33, !tbaa !9
  %35 = getelementptr inbounds i8, ptr @_vx, i32 %32
  %36 = load i16, ptr %35, !tbaa !9
  %37 = add i16 %34, %36
  store i16 %37, ptr %33, !tbaa !9
  %38 = getelementptr inbounds i8, ptr @_py, i32 %32
  %39 = load i16, ptr %38, !tbaa !9
  %40 = getelementptr inbounds i8, ptr @_vy, i32 %32
  %41 = load i16, ptr %40, !tbaa !9
  %42 = add i16 %39, %41
  store i16 %42, ptr %38, !tbaa !9
  %43 = getelementptr inbounds i8, ptr @_pz, i32 %32
  %44 = load i16, ptr %43, !tbaa !9
  %45 = getelementptr inbounds i8, ptr @_vz, i32 %32
  %46 = load i16, ptr %45, !tbaa !9
  %47 = add i16 %44, %46
  store i16 %47, ptr %43, !tbaa !9
  %48 = add i16 %26, 1
  br label %25

b8:
  %49 = phi i32 [ 0, %b6 ], [ %68, %b10 ]
  %50 = phi i16 [ 0, %b6 ], [ %69, %b10 ]
  %51 = sext i16 %50 to i32
  %52 = icmp sle i16 %50, 100
  br i1 %52, label %b10, label %b9

b9:
  %53 = phi i32 [ %49, %b8 ]
  ret i32 %53

b10:
  %54 = mul i32 %51, 2
  %55 = getelementptr inbounds i8, ptr @_px, i32 %54
  %56 = load i16, ptr %55, !tbaa !9
  %57 = sext i16 %56 to i32
  %58 = add nsw i32 %49, %57
  %59 = getelementptr inbounds i8, ptr @_py, i32 %54
  %60 = load i16, ptr %59, !tbaa !9
  %61 = sext i16 %60 to i32
  %62 = mul nsw i32 3, %61
  %63 = add nsw i32 %58, %62
  %64 = getelementptr inbounds i8, ptr @_pz, i32 %54
  %65 = load i16, ptr %64, !tbaa !9
  %66 = sext i16 %65 to i32
  %67 = mul nsw i32 7, %66
  %68 = add nsw i32 %63, %67
  %69 = add i16 %50, 1
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
