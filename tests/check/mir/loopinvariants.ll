; RUN: llrm-mir %s
; CHECK: define {{.*}} @"_bench_nbody_fixed@3"(


@_pos_x = internal global [24 x i8] zeroinitializer
@_pos_y = internal global [24 x i8] zeroinitializer
@_vel_x = internal global [24 x i8] zeroinitializer
@_vel_y = internal global [24 x i8] zeroinitializer

define regparm3cc i32 @"_bench_nbody_fixed@3"() addrspace(1) memory(readwrite, argmem: none, inaccessiblemem: none) norecurse willreturn {
b1:
  br label %b4

b3:
  br label %b7

b4:
  %lsr.iv9 = phi i32 [ -7680, %b1 ], [ %lsr.iv.next, %b4 ]
  %lsr.iv11 = phi i32 [ -6144, %b1 ], [ %lsr.iv.next1, %b4 ]
  %lsr.iv21 = phi i16 [ -24, %b1 ], [ %lsr.iv.next2, %b4 ]
  %0 = getelementptr i8, ptr @_pos_x, i16 %lsr.iv21
  %1 = getelementptr i8, ptr %0, i16 24
  store i32 %lsr.iv9, ptr %1, !tbaa !11
  %2 = getelementptr i8, ptr @_pos_y, i16 %lsr.iv21
  %3 = getelementptr i8, ptr %2, i16 24
  store i32 %lsr.iv11, ptr %3, !tbaa !11
  %4 = getelementptr i8, ptr @_vel_x, i16 %lsr.iv21
  %5 = getelementptr i8, ptr %4, i16 24
  store i32 0, ptr %5, !tbaa !11
  %6 = getelementptr i8, ptr @_vel_y, i16 %lsr.iv21
  %7 = getelementptr i8, ptr %6, i16 24
  store i32 0, ptr %7, !tbaa !11
  %lsr.iv.next = add i32 %lsr.iv9, 3584
  %lsr.iv.next1 = add i32 %lsr.iv11, 2560
  %lsr.iv.next2 = add i16 %lsr.iv21, 4
  %8 = icmp ne i16 %lsr.iv.next2, 0
  br i1 %8, label %b4, label %b3

b6:
  br label %b21

b7:
  %lsr.iv81 = phi i16 [ -1000, %b3 ], [ %lsr.iv.next8, %b17 ]
  br label %b10

b9:
  br label %b18

b10:
  %lsr.iv71 = phi i16 [ -6, %b7 ], [ %lsr.iv.next7, %b12 ]
  %9 = shl i16 %lsr.iv71, 2
  %10 = getelementptr i8, ptr @_pos_x, i16 %9
  %11 = getelementptr i8, ptr %10, i16 24
  %12 = load i32, ptr %11, !tbaa !11
  %13 = getelementptr i8, ptr @_pos_y, i16 %9
  %14 = getelementptr i8, ptr %13, i16 24
  %15 = load i32, ptr %14, !tbaa !11
  br label %b13

b12:
  %16 = phi i32 [ %46, %b14 ]
  %17 = phi i32 [ %45, %b14 ]
  %18 = shl i16 %lsr.iv71, 2
  %19 = getelementptr i8, ptr @_vel_x, i16 %18
  %20 = getelementptr i8, ptr %19, i16 24
  %21 = load i32, ptr %20, !tbaa !11
  %22 = add nsw i32 %21, %16
  %23 = getelementptr i8, ptr @_vel_y, i16 %18
  %24 = getelementptr i8, ptr %23, i16 24
  %25 = load i32, ptr %24, !tbaa !11
  %26 = add nsw i32 %25, %17
  %27 = ashr i32 %22, 31
  %28 = and i32 %27, 15
  %29 = add i32 %22, %28
  %30 = ashr i32 %29, 4
  %31 = sub nsw i32 %22, %30
  %32 = getelementptr i8, ptr @_vel_x, i16 %18
  %33 = getelementptr i8, ptr %32, i16 24
  store i32 %31, ptr %33, !tbaa !11
  %34 = ashr i32 %26, 31
  %35 = and i32 %34, 15
  %36 = add i32 %26, %35
  %37 = ashr i32 %36, 4
  %38 = sub nsw i32 %26, %37
  %39 = getelementptr i8, ptr @_vel_y, i16 %18
  %40 = getelementptr i8, ptr %39, i16 24
  store i32 %38, ptr %40, !tbaa !11
  %lsr.iv.next7 = add i16 %lsr.iv71, 1
  %41 = icmp ne i16 %lsr.iv.next7, 0
  br i1 %41, label %b10, label %b9

b13:
  %42 = phi i32 [ 0, %b10 ], [ %46, %b14 ]
  %43 = phi i32 [ 0, %b10 ], [ %45, %b14 ]
  %lsr.iv61 = phi i16 [ -6, %b10 ], [ %lsr.iv.next6, %b14 ]
  %44 = icmp ne i16 %lsr.iv61, %lsr.iv71
  br i1 %44, label %b15, label %b14

b14:
  %45 = phi i32 [ %43, %b13 ], [ %78, %b15 ]
  %46 = phi i32 [ %42, %b13 ], [ %72, %b15 ]
  %lsr.iv.next6 = add i16 %lsr.iv61, 1
  %47 = icmp ne i16 %lsr.iv.next6, 0
  br i1 %47, label %b13, label %b12

b15:
  %48 = shl i16 %lsr.iv61, 2
  %49 = getelementptr i8, ptr @_pos_x, i16 %48
  %50 = getelementptr i8, ptr %49, i16 24
  %51 = load i32, ptr %50, !tbaa !11
  %52 = sub nsw i32 %51, %12
  %53 = getelementptr i8, ptr @_pos_y, i16 %48
  %54 = getelementptr i8, ptr %53, i16 24
  %55 = load i32, ptr %54, !tbaa !11
  %56 = sub nsw i32 %55, %15
  %57 = mul nsw i32 %52, %52
  %58 = mul nsw i32 %56, %56
  %59 = add nsw i32 %57, %58
  %60 = add nsw i32 %59, 262144
  %61 = ashr i32 %60, 31
  %62 = and i32 %61, 262143
  %63 = add i32 %60, %62
  %64 = ashr i32 %63, 18
  %65 = add nsw i32 %64, 1
  %66 = sdiv i32 512, %65
  %67 = mul nsw i32 %52, %66
  %68 = ashr i32 %67, 31
  %69 = and i32 %68, 511
  %70 = add i32 %67, %69
  %71 = ashr i32 %70, 9
  %72 = add nsw i32 %42, %71
  %73 = mul nsw i32 %56, %66
  %74 = ashr i32 %73, 31
  %75 = and i32 %74, 511
  %76 = add i32 %73, %75
  %77 = ashr i32 %76, 9
  %78 = add nsw i32 %43, %77
  br label %b14

b17:
  %lsr.iv.next8 = add i16 %lsr.iv81, 1
  %79 = icmp ne i16 %lsr.iv.next8, 0
  br i1 %79, label %b7, label %b6

b18:
  %lsr.iv31 = phi i16 [ -24, %b9 ], [ %lsr.iv.next3, %b18 ]
  %80 = getelementptr i8, ptr @_pos_x, i16 %lsr.iv31
  %81 = getelementptr i8, ptr %80, i16 24
  %82 = load i32, ptr %81, !tbaa !11
  %83 = getelementptr i8, ptr @_vel_x, i16 %lsr.iv31
  %84 = getelementptr i8, ptr %83, i16 24
  %85 = load i32, ptr %84, !tbaa !11
  %86 = add nsw i32 %82, %85
  %87 = getelementptr i8, ptr @_pos_x, i16 %lsr.iv31
  %88 = getelementptr i8, ptr %87, i16 24
  store i32 %86, ptr %88, !tbaa !11
  %89 = getelementptr i8, ptr @_pos_y, i16 %lsr.iv31
  %90 = getelementptr i8, ptr %89, i16 24
  %91 = load i32, ptr %90, !tbaa !11
  %92 = getelementptr i8, ptr @_vel_y, i16 %lsr.iv31
  %93 = getelementptr i8, ptr %92, i16 24
  %94 = load i32, ptr %93, !tbaa !11
  %95 = add nsw i32 %91, %94
  %96 = getelementptr i8, ptr @_pos_y, i16 %lsr.iv31
  %97 = getelementptr i8, ptr %96, i16 24
  store i32 %95, ptr %97, !tbaa !11
  %lsr.iv.next3 = add i16 %lsr.iv31, 4
  %98 = icmp ne i16 %lsr.iv.next3, 0
  br i1 %98, label %b18, label %b17

b20:
  %99 = phi i32 [ %120, %b21 ]
  ret i32 %99

b21:
  %100 = phi i32 [ 0, %b6 ], [ %120, %b21 ]
  %lsr.iv41 = phi i32 [ 0, %b6 ], [ %lsr.iv.next4, %b21 ]
  %lsr.iv51 = phi i16 [ -24, %b6 ], [ %lsr.iv.next5, %b21 ]
  %101 = getelementptr i8, ptr @_pos_x, i16 %lsr.iv51
  %102 = getelementptr i8, ptr %101, i16 24
  %103 = load i32, ptr %102, !tbaa !11
  %104 = getelementptr i8, ptr @_pos_y, i16 %lsr.iv51
  %105 = getelementptr i8, ptr %104, i16 24
  %106 = load i32, ptr %105, !tbaa !11
  %107 = mul nsw i32 %106, 3
  %108 = add nsw i32 %103, %107
  %109 = getelementptr i8, ptr @_vel_x, i16 %lsr.iv51
  %110 = getelementptr i8, ptr %109, i16 24
  %111 = load i32, ptr %110, !tbaa !11
  %112 = mul nsw i32 %111, 5
  %113 = add nsw i32 %108, %112
  %114 = getelementptr i8, ptr @_vel_y, i16 %lsr.iv51
  %115 = getelementptr i8, ptr %114, i16 24
  %116 = load i32, ptr %115, !tbaa !11
  %117 = mul nsw i32 %116, 7
  %118 = add nsw i32 %113, %117
  %lsr.iv.next4 = add i32 %lsr.iv41, 1
  %119 = mul nsw i32 %lsr.iv.next4, %118
  %120 = add nsw i32 %100, %119
  %lsr.iv.next5 = add i16 %lsr.iv51, 4
  %121 = icmp ne i16 %lsr.iv.next5, 0
  br i1 %121, label %b21, label %b20
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
