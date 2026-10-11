target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

@_vx = internal global [202 x i8] zeroinitializer
@_vy = internal global [202 x i8] zeroinitializer
@_vz = internal global [202 x i8] zeroinitializer
@_px = internal global [202 x i8] zeroinitializer
@_py = internal global [202 x i8] zeroinitializer
@_pz = internal global [202 x i8] zeroinitializer

define watcallcc i32 @bench_particle_() memory(readwrite, argmem: none, inaccessiblemem: none) norecurse willreturn {
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
  %19 = phi i16 [ 1, %b3 ], [ %27, %26 ]
  %20 = icmp sle i16 %19, 100
  br i1 %20, label %21, label %b6

b6:
  br label %b8

21:
  br label %22

22:
  %23 = phi i16 [ 0, %21 ], [ %45, %28 ]
  %24 = sext i16 %23 to i32
  %25 = icmp sle i16 %23, 100
  br i1 %25, label %28, label %26

26:
  %27 = add i16 %19, 1
  br label %b5

28:
  %29 = mul i32 %24, 2
  %30 = getelementptr inbounds i8, ptr @_px, i32 %29
  %31 = load i16, ptr %30, !tbaa !9
  %32 = getelementptr inbounds i8, ptr @_vx, i32 %29
  %33 = load i16, ptr %32, !tbaa !9
  %34 = add i16 %31, %33
  store i16 %34, ptr %30, !tbaa !9
  %35 = getelementptr inbounds i8, ptr @_py, i32 %29
  %36 = load i16, ptr %35, !tbaa !9
  %37 = getelementptr inbounds i8, ptr @_vy, i32 %29
  %38 = load i16, ptr %37, !tbaa !9
  %39 = add i16 %36, %38
  store i16 %39, ptr %35, !tbaa !9
  %40 = getelementptr inbounds i8, ptr @_pz, i32 %29
  %41 = load i16, ptr %40, !tbaa !9
  %42 = getelementptr inbounds i8, ptr @_vz, i32 %29
  %43 = load i16, ptr %42, !tbaa !9
  %44 = add i16 %41, %43
  store i16 %44, ptr %40, !tbaa !9
  %45 = add i16 %23, 1
  br label %22

b8:
  %46 = phi i32 [ 0, %b6 ], [ %65, %b10 ]
  %47 = phi i16 [ 0, %b6 ], [ %66, %b10 ]
  %48 = sext i16 %47 to i32
  %49 = icmp sle i16 %47, 100
  br i1 %49, label %b10, label %b9

b9:
  %50 = phi i32 [ %46, %b8 ]
  ret i32 %50

b10:
  %51 = mul i32 %48, 2
  %52 = getelementptr inbounds i8, ptr @_px, i32 %51
  %53 = load i16, ptr %52, !tbaa !9
  %54 = sext i16 %53 to i32
  %55 = add nsw i32 %46, %54
  %56 = getelementptr inbounds i8, ptr @_py, i32 %51
  %57 = load i16, ptr %56, !tbaa !9
  %58 = sext i16 %57 to i32
  %59 = mul nsw i32 3, %58
  %60 = add nsw i32 %55, %59
  %61 = getelementptr inbounds i8, ptr @_pz, i32 %51
  %62 = load i16, ptr %61, !tbaa !9
  %63 = sext i16 %62 to i32
  %64 = mul nsw i32 7, %63
  %65 = add nsw i32 %60, %64
  %66 = add i16 %47, 1
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
