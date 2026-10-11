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
  %6 = load i16, ptr %1, !tbaa !9
  %7 = sext i16 %6 to i32
  %8 = icmp sle i32 %7, 100
  %9 = zext i1 %8 to i32
  br i1 %8, label %b4, label %b3

b3:
  %10 = trunc i32 1 to i16
  store i16 1, ptr %2, !tbaa !9
  br label %b5

b4:
  %11 = load i16, ptr %1, !tbaa !9
  %12 = sext i16 %11 to i32
  %13 = sub nsw i32 %12, 50
  %14 = trunc i32 %13 to i16
  store i16 %14, ptr %3, !tbaa !9
  %15 = load i16, ptr %3, !tbaa !9
  %16 = load i16, ptr %1, !tbaa !9
  %17 = sext i16 %16 to i32
  %18 = mul i32 %17, 2
  %19 = getelementptr inbounds i8, ptr @_px, i32 %18
  store i16 %15, ptr %19, !tbaa !9
  %20 = load i16, ptr %3, !tbaa !9
  %21 = sext i16 %20 to i32
  %22 = mul nsw i32 2, %21
  %23 = trunc i32 %22 to i16
  %24 = load i16, ptr %1, !tbaa !9
  %25 = sext i16 %24 to i32
  %26 = mul i32 %25, 2
  %27 = getelementptr inbounds i8, ptr @_py, i32 %26
  store i16 %23, ptr %27, !tbaa !9
  %28 = load i16, ptr %3, !tbaa !9
  %29 = sext i16 %28 to i32
  %30 = sub nsw i32 0, %29
  %31 = trunc i32 %30 to i16
  %32 = load i16, ptr %1, !tbaa !9
  %33 = sext i16 %32 to i32
  %34 = mul i32 %33, 2
  %35 = getelementptr inbounds i8, ptr @_pz, i32 %34
  store i16 %31, ptr %35, !tbaa !9
  %36 = load i16, ptr %3, !tbaa !9
  %37 = sext i16 %36 to i32
  %38 = and i32 %37, 7
  %39 = trunc i32 %38 to i16
  %40 = load i16, ptr %1, !tbaa !9
  %41 = sext i16 %40 to i32
  %42 = mul i32 %41, 2
  %43 = getelementptr inbounds i8, ptr @_vx, i32 %42
  store i16 %39, ptr %43, !tbaa !9
  %44 = load i16, ptr %3, !tbaa !9
  %45 = sext i16 %44 to i32
  %46 = and i32 %45, 3
  %47 = sub nsw i32 3, %46
  %48 = trunc i32 %47 to i16
  %49 = load i16, ptr %1, !tbaa !9
  %50 = sext i16 %49 to i32
  %51 = mul i32 %50, 2
  %52 = getelementptr inbounds i8, ptr @_vy, i32 %51
  store i16 %48, ptr %52, !tbaa !9
  %53 = trunc i32 1 to i16
  %54 = load i16, ptr %1, !tbaa !9
  %55 = sext i16 %54 to i32
  %56 = mul i32 %55, 2
  %57 = getelementptr inbounds i8, ptr @_vz, i32 %56
  store i16 1, ptr %57, !tbaa !9
  %58 = load i16, ptr %1, !tbaa !9
  %59 = sext i16 %58 to i32
  %60 = add nsw i32 %59, 1
  %61 = trunc i32 %60 to i16
  store i16 %61, ptr %1, !tbaa !9
  br label %b2

b5:
  %62 = load i16, ptr %2, !tbaa !9
  %63 = sext i16 %62 to i32
  %64 = icmp sle i32 %63, 100
  %65 = zext i1 %64 to i32
  br i1 %64, label %b7, label %b6

b6:
  store i32 0, ptr %4, !tbaa !11
  %66 = trunc i32 0 to i16
  store i16 0, ptr %1, !tbaa !9
  br label %b8

b7:
  call watcallcc void @advance_(i16 50)
  %67 = load i16, ptr %2, !tbaa !9
  %68 = sext i16 %67 to i32
  %69 = add nsw i32 %68, 1
  %70 = trunc i32 %69 to i16
  store i16 %70, ptr %2, !tbaa !9
  br label %b5

b8:
  %71 = load i16, ptr %1, !tbaa !9
  %72 = sext i16 %71 to i32
  %73 = icmp sle i32 %72, 100
  %74 = zext i1 %73 to i32
  br i1 %73, label %b10, label %b9

b9:
  %75 = load i32, ptr %4, !tbaa !11
  store i32 %75, ptr %0, !tbaa !11
  %76 = load i32, ptr %0, !tbaa !11
  ret i32 %76

b10:
  %77 = load i32, ptr %4, !tbaa !11
  %78 = load i16, ptr %1, !tbaa !9
  %79 = sext i16 %78 to i32
  %80 = mul i32 %79, 2
  %81 = getelementptr inbounds i8, ptr @_px, i32 %80
  %82 = load i16, ptr %81, !tbaa !9
  %83 = sext i16 %82 to i32
  %84 = add nsw i32 %77, %83
  %85 = load i16, ptr %1, !tbaa !9
  %86 = sext i16 %85 to i32
  %87 = mul i32 %86, 2
  %88 = getelementptr inbounds i8, ptr @_py, i32 %87
  %89 = load i16, ptr %88, !tbaa !9
  %90 = sext i16 %89 to i32
  %91 = mul nsw i32 3, %90
  %92 = add nsw i32 %84, %91
  %93 = load i16, ptr %1, !tbaa !9
  %94 = sext i16 %93 to i32
  %95 = mul i32 %94, 2
  %96 = getelementptr inbounds i8, ptr @_pz, i32 %95
  %97 = load i16, ptr %96, !tbaa !9
  %98 = sext i16 %97 to i32
  %99 = mul nsw i32 7, %98
  %100 = add nsw i32 %92, %99
  store i32 %100, ptr %4, !tbaa !11
  %101 = load i16, ptr %1, !tbaa !9
  %102 = sext i16 %101 to i32
  %103 = add nsw i32 %102, 1
  %104 = trunc i32 %103 to i16
  store i16 %104, ptr %1, !tbaa !9
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
