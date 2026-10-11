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
  %19 = phi i16 [ 1, %b3 ], [ %49, %48 ]
  %20 = icmp sle i16 %19, 100
  br i1 %20, label %b7, label %b6

b6:
  br label %b8

b7:
  br label %21

21:
  %22 = sext i16 50 to i32
  %23 = sub i16 50, 50
  %24 = add nsw i32 50, 50
  br label %25

25:
  %26 = phi i16 [ 0, %21 ], [ %47, %30 ]
  %27 = sext i16 %26 to i32
  %28 = icmp sle i32 %27, 100
  br i1 %28, label %30, label %29

29:
  br label %48

30:
  %31 = mul i32 %27, 2
  %32 = getelementptr inbounds i8, ptr @_px, i32 %31
  %33 = load i16, ptr %32, !tbaa !9
  %34 = getelementptr inbounds i8, ptr @_vx, i32 %31
  %35 = load i16, ptr %34, !tbaa !9
  %36 = add i16 %33, %35
  store i16 %36, ptr %32, !tbaa !9
  %37 = getelementptr inbounds i8, ptr @_py, i32 %31
  %38 = load i16, ptr %37, !tbaa !9
  %39 = getelementptr inbounds i8, ptr @_vy, i32 %31
  %40 = load i16, ptr %39, !tbaa !9
  %41 = add i16 %38, %40
  store i16 %41, ptr %37, !tbaa !9
  %42 = getelementptr inbounds i8, ptr @_pz, i32 %31
  %43 = load i16, ptr %42, !tbaa !9
  %44 = getelementptr inbounds i8, ptr @_vz, i32 %31
  %45 = load i16, ptr %44, !tbaa !9
  %46 = add i16 %43, %45
  store i16 %46, ptr %42, !tbaa !9
  %47 = add i16 %26, 1
  br label %25

48:
  %49 = add i16 %19, 1
  br label %b5

b8:
  %50 = phi i32 [ 0, %b6 ], [ %69, %b10 ]
  %51 = phi i16 [ 0, %b6 ], [ %70, %b10 ]
  %52 = sext i16 %51 to i32
  %53 = icmp sle i16 %51, 100
  br i1 %53, label %b10, label %b9

b9:
  %54 = phi i32 [ %50, %b8 ]
  ret i32 %54

b10:
  %55 = mul i32 %52, 2
  %56 = getelementptr inbounds i8, ptr @_px, i32 %55
  %57 = load i16, ptr %56, !tbaa !9
  %58 = sext i16 %57 to i32
  %59 = add nsw i32 %50, %58
  %60 = getelementptr inbounds i8, ptr @_py, i32 %55
  %61 = load i16, ptr %60, !tbaa !9
  %62 = sext i16 %61 to i32
  %63 = mul nsw i32 3, %62
  %64 = add nsw i32 %59, %63
  %65 = getelementptr inbounds i8, ptr @_pz, i32 %55
  %66 = load i16, ptr %65, !tbaa !9
  %67 = sext i16 %66 to i32
  %68 = mul nsw i32 7, %67
  %69 = add nsw i32 %64, %68
  %70 = add i16 %51, 1
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
