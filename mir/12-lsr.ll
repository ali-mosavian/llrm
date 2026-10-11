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
  %lsr.iv2 = phi i32 [ -50, %b1 ], [ %lsr.iv.next2, %b4 ]
  %lsr.iv1 = phi i16 [ -100, %b1 ], [ %lsr.iv.next1, %b4 ]
  %lsr.iv = phi i16 [ -50, %b1 ], [ %lsr.iv.next, %b4 ]
  %0 = icmp ne i16 %lsr.iv, 51
  br i1 %0, label %b4, label %b3

b3:
  br label %b5

b4:
  %1 = shl i32 %lsr.iv2, 1
  %2 = getelementptr i8, ptr @_px, i32 %1
  %3 = getelementptr i8, ptr %2, i32 100
  store i16 %lsr.iv, ptr %3, !tbaa !9
  %4 = getelementptr i8, ptr @_py, i32 %1
  %5 = getelementptr i8, ptr %4, i32 100
  store i16 %lsr.iv1, ptr %5, !tbaa !9
  %6 = sub i16 0, %lsr.iv
  %7 = getelementptr i8, ptr @_pz, i32 %1
  %8 = getelementptr i8, ptr %7, i32 100
  store i16 %6, ptr %8, !tbaa !9
  %9 = and i16 %lsr.iv, 7
  %10 = getelementptr i8, ptr @_vx, i32 %1
  %11 = getelementptr i8, ptr %10, i32 100
  store i16 %9, ptr %11, !tbaa !9
  %12 = and i32 %lsr.iv2, 3
  %13 = sub nsw i32 3, %12
  %14 = trunc i32 %13 to i16
  %15 = getelementptr i8, ptr @_vy, i32 %1
  %16 = getelementptr i8, ptr %15, i32 100
  store i16 %14, ptr %16, !tbaa !9
  %17 = getelementptr i8, ptr @_vz, i32 %1
  %18 = getelementptr i8, ptr %17, i32 100
  store i16 1, ptr %18, !tbaa !9
  %lsr.iv.next = add i16 %lsr.iv, 1
  %lsr.iv.next1 = add i16 %lsr.iv1, 2
  %lsr.iv.next2 = add i32 %lsr.iv2, 1
  br label %b2

b5:
  %lsr.iv5 = phi i16 [ -100, %b3 ], [ %lsr.iv.next5, %23 ]
  %19 = icmp ne i16 %lsr.iv5, 0
  br i1 %19, label %20, label %b6

b6:
  br label %b8

20:
  br label %21

21:
  %lsr.iv4 = phi i32 [ -202, %20 ], [ %lsr.iv.next4, %24 ]
  %22 = icmp ne i32 %lsr.iv4, 0
  br i1 %22, label %24, label %23

23:
  %lsr.iv.next5 = add i16 %lsr.iv5, 1
  br label %b5

24:
  %25 = getelementptr i8, ptr @_px, i32 %lsr.iv4
  %26 = getelementptr i8, ptr %25, i32 202
  %27 = load i16, ptr %26, !tbaa !9
  %28 = getelementptr i8, ptr @_vx, i32 %lsr.iv4
  %29 = getelementptr i8, ptr %28, i32 202
  %30 = load i16, ptr %29, !tbaa !9
  %31 = add i16 %27, %30
  %32 = getelementptr i8, ptr @_px, i32 %lsr.iv4
  %33 = getelementptr i8, ptr %32, i32 202
  store i16 %31, ptr %33, !tbaa !9
  %34 = getelementptr i8, ptr @_py, i32 %lsr.iv4
  %35 = getelementptr i8, ptr %34, i32 202
  %36 = load i16, ptr %35, !tbaa !9
  %37 = getelementptr i8, ptr @_vy, i32 %lsr.iv4
  %38 = getelementptr i8, ptr %37, i32 202
  %39 = load i16, ptr %38, !tbaa !9
  %40 = add i16 %36, %39
  %41 = getelementptr i8, ptr @_py, i32 %lsr.iv4
  %42 = getelementptr i8, ptr %41, i32 202
  store i16 %40, ptr %42, !tbaa !9
  %43 = getelementptr i8, ptr @_pz, i32 %lsr.iv4
  %44 = getelementptr i8, ptr %43, i32 202
  %45 = load i16, ptr %44, !tbaa !9
  %46 = getelementptr i8, ptr @_vz, i32 %lsr.iv4
  %47 = getelementptr i8, ptr %46, i32 202
  %48 = load i16, ptr %47, !tbaa !9
  %49 = add i16 %45, %48
  %50 = getelementptr i8, ptr @_pz, i32 %lsr.iv4
  %51 = getelementptr i8, ptr %50, i32 202
  store i16 %49, ptr %51, !tbaa !9
  %lsr.iv.next4 = add i32 %lsr.iv4, 2
  br label %21

b8:
  %lsr.iv3 = phi i32 [ -202, %b6 ], [ %lsr.iv.next3, %b10 ]
  %52 = phi i32 [ 0, %b6 ], [ %71, %b10 ]
  %53 = icmp ne i32 %lsr.iv3, 0
  br i1 %53, label %b10, label %b9

b9:
  %54 = phi i32 [ %52, %b8 ]
  ret i32 %54

b10:
  %55 = getelementptr i8, ptr @_px, i32 %lsr.iv3
  %56 = getelementptr i8, ptr %55, i32 202
  %57 = load i16, ptr %56, !tbaa !9
  %58 = sext i16 %57 to i32
  %59 = add nsw i32 %52, %58
  %60 = getelementptr i8, ptr @_py, i32 %lsr.iv3
  %61 = getelementptr i8, ptr %60, i32 202
  %62 = load i16, ptr %61, !tbaa !9
  %63 = sext i16 %62 to i32
  %64 = mul nsw i32 3, %63
  %65 = add nsw i32 %59, %64
  %66 = getelementptr i8, ptr @_pz, i32 %lsr.iv3
  %67 = getelementptr i8, ptr %66, i32 202
  %68 = load i16, ptr %67, !tbaa !9
  %69 = sext i16 %68 to i32
  %70 = mul nsw i32 7, %69
  %71 = add nsw i32 %65, %70
  %lsr.iv.next3 = add i32 %lsr.iv3, 2
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
