target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

@_vx = internal global [202 x i8] zeroinitializer
@_vy = internal global [202 x i8] zeroinitializer
@_vz = internal global [202 x i8] zeroinitializer
@_px = internal global [202 x i8] zeroinitializer
@_py = internal global [202 x i8] zeroinitializer
@_pz = internal global [202 x i8] zeroinitializer

define watcallcc i32 @bench_particle_() memory(readwrite, argmem: none, inaccessiblemem: none) norecurse willreturn {
b1:
  br label %b4

b3:
  br label %19

b4:
  %lsr.iv6 = phi i16 [ -50, %b1 ], [ %lsr.iv.next, %b4 ]
  %lsr.iv11 = phi i16 [ -100, %b1 ], [ %lsr.iv.next1, %b4 ]
  %lsr.iv21 = phi i32 [ -50, %b1 ], [ %lsr.iv.next2, %b4 ]
  %0 = shl i32 %lsr.iv21, 1
  %1 = getelementptr i8, ptr @_px, i32 %0
  %2 = getelementptr i8, ptr %1, i32 100
  store i16 %lsr.iv6, ptr %2, !tbaa !9
  %3 = getelementptr i8, ptr @_py, i32 %0
  %4 = getelementptr i8, ptr %3, i32 100
  store i16 %lsr.iv11, ptr %4, !tbaa !9
  %5 = sub i16 0, %lsr.iv6
  %6 = getelementptr i8, ptr @_pz, i32 %0
  %7 = getelementptr i8, ptr %6, i32 100
  store i16 %5, ptr %7, !tbaa !9
  %8 = and i16 %lsr.iv6, 7
  %9 = getelementptr i8, ptr @_vx, i32 %0
  %10 = getelementptr i8, ptr %9, i32 100
  store i16 %8, ptr %10, !tbaa !9
  %11 = and i32 %lsr.iv21, 3
  %12 = sub nsw i32 3, %11
  %13 = trunc i32 %12 to i16
  %14 = getelementptr i8, ptr @_vy, i32 %0
  %15 = getelementptr i8, ptr %14, i32 100
  store i16 %13, ptr %15, !tbaa !9
  %16 = getelementptr i8, ptr @_vz, i32 %0
  %17 = getelementptr i8, ptr %16, i32 100
  store i16 1, ptr %17, !tbaa !9
  %lsr.iv.next = add i16 %lsr.iv6, 1
  %lsr.iv.next1 = add i16 %lsr.iv11, 2
  %lsr.iv.next2 = add i32 %lsr.iv21, 1
  %18 = icmp ne i16 %lsr.iv.next, 51
  br i1 %18, label %b4, label %b3

b6:
  br label %b10

19:
  %lsr.iv51 = phi i16 [ -100, %b3 ], [ %lsr.iv.next5, %20 ]
  br label %22

20:
  %lsr.iv.next5 = add i16 %lsr.iv51, 1
  %21 = icmp ne i16 %lsr.iv.next5, 0
  br i1 %21, label %19, label %b6

22:
  %lsr.iv41 = phi i32 [ -202, %19 ], [ %lsr.iv.next4, %22 ]
  %23 = getelementptr i8, ptr @_px, i32 %lsr.iv41
  %24 = getelementptr i8, ptr %23, i32 202
  %25 = load i16, ptr %24, !tbaa !9
  %26 = getelementptr i8, ptr @_vx, i32 %lsr.iv41
  %27 = getelementptr i8, ptr %26, i32 202
  %28 = load i16, ptr %27, !tbaa !9
  %29 = add i16 %25, %28
  %30 = getelementptr i8, ptr @_px, i32 %lsr.iv41
  %31 = getelementptr i8, ptr %30, i32 202
  store i16 %29, ptr %31, !tbaa !9
  %32 = getelementptr i8, ptr @_py, i32 %lsr.iv41
  %33 = getelementptr i8, ptr %32, i32 202
  %34 = load i16, ptr %33, !tbaa !9
  %35 = getelementptr i8, ptr @_vy, i32 %lsr.iv41
  %36 = getelementptr i8, ptr %35, i32 202
  %37 = load i16, ptr %36, !tbaa !9
  %38 = add i16 %34, %37
  %39 = getelementptr i8, ptr @_py, i32 %lsr.iv41
  %40 = getelementptr i8, ptr %39, i32 202
  store i16 %38, ptr %40, !tbaa !9
  %41 = getelementptr i8, ptr @_pz, i32 %lsr.iv41
  %42 = getelementptr i8, ptr %41, i32 202
  %43 = load i16, ptr %42, !tbaa !9
  %44 = getelementptr i8, ptr @_vz, i32 %lsr.iv41
  %45 = getelementptr i8, ptr %44, i32 202
  %46 = load i16, ptr %45, !tbaa !9
  %47 = add i16 %43, %46
  %48 = getelementptr i8, ptr @_pz, i32 %lsr.iv41
  %49 = getelementptr i8, ptr %48, i32 202
  store i16 %47, ptr %49, !tbaa !9
  %lsr.iv.next4 = add i32 %lsr.iv41, 2
  %50 = icmp ne i32 %lsr.iv.next4, 0
  br i1 %50, label %22, label %20

b9:
  %51 = phi i32 [ %69, %b10 ]
  ret i32 %51

b10:
  %52 = phi i32 [ 0, %b6 ], [ %69, %b10 ]
  %lsr.iv31 = phi i32 [ -202, %b6 ], [ %lsr.iv.next3, %b10 ]
  %53 = getelementptr i8, ptr @_px, i32 %lsr.iv31
  %54 = getelementptr i8, ptr %53, i32 202
  %55 = load i16, ptr %54, !tbaa !9
  %56 = sext i16 %55 to i32
  %57 = add nsw i32 %52, %56
  %58 = getelementptr i8, ptr @_py, i32 %lsr.iv31
  %59 = getelementptr i8, ptr %58, i32 202
  %60 = load i16, ptr %59, !tbaa !9
  %61 = sext i16 %60 to i32
  %62 = mul nsw i32 3, %61
  %63 = add nsw i32 %57, %62
  %64 = getelementptr i8, ptr @_pz, i32 %lsr.iv31
  %65 = getelementptr i8, ptr %64, i32 202
  %66 = load i16, ptr %65, !tbaa !9
  %67 = sext i16 %66 to i32
  %68 = mul nsw i32 7, %67
  %69 = add nsw i32 %63, %68
  %lsr.iv.next3 = add i32 %lsr.iv31, 2
  %70 = icmp ne i32 %lsr.iv.next3, 0
  br i1 %70, label %b10, label %b9
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
