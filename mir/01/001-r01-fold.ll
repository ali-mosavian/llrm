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
  %4 = load i16, ptr %1, !tbaa !9
  %5 = sext i16 %4 to i32
  %6 = sub nsw i32 50, %5
  %7 = trunc i32 %6 to i16
  store i16 %7, ptr %3, !tbaa !9
  br label %b2

b2:
  %8 = load i16, ptr %3, !tbaa !9
  %9 = sext i16 %8 to i32
  %10 = load i16, ptr %1, !tbaa !9
  %11 = sext i16 %10 to i32
  %12 = add nsw i32 50, %11
  %13 = icmp sle i32 %9, %12
  %14 = zext i1 %13 to i32
  br i1 %13, label %b4, label %b3

b3:
  ret void

b4:
  %15 = load i16, ptr %3, !tbaa !9
  %16 = sext i16 %15 to i32
  %17 = mul i32 %16, 2
  %18 = getelementptr inbounds i8, ptr @_px, i32 %17
  %19 = load i16, ptr %18, !tbaa !9
  %20 = load i16, ptr %3, !tbaa !9
  %21 = sext i16 %20 to i32
  %22 = mul i32 %21, 2
  %23 = getelementptr inbounds i8, ptr @_vx, i32 %22
  %24 = load i16, ptr %23, !tbaa !9
  %25 = sext i16 %19 to i32
  %26 = sext i16 %24 to i32
  %27 = add nsw i32 %25, %26
  %28 = trunc i32 %27 to i16
  %29 = load i16, ptr %3, !tbaa !9
  %30 = sext i16 %29 to i32
  %31 = mul i32 %30, 2
  %32 = getelementptr inbounds i8, ptr @_px, i32 %31
  store i16 %28, ptr %32, !tbaa !9
  %33 = load i16, ptr %3, !tbaa !9
  %34 = sext i16 %33 to i32
  %35 = mul i32 %34, 2
  %36 = getelementptr inbounds i8, ptr @_py, i32 %35
  %37 = load i16, ptr %36, !tbaa !9
  %38 = load i16, ptr %3, !tbaa !9
  %39 = sext i16 %38 to i32
  %40 = mul i32 %39, 2
  %41 = getelementptr inbounds i8, ptr @_vy, i32 %40
  %42 = load i16, ptr %41, !tbaa !9
  %43 = sext i16 %37 to i32
  %44 = sext i16 %42 to i32
  %45 = add nsw i32 %43, %44
  %46 = trunc i32 %45 to i16
  %47 = load i16, ptr %3, !tbaa !9
  %48 = sext i16 %47 to i32
  %49 = mul i32 %48, 2
  %50 = getelementptr inbounds i8, ptr @_py, i32 %49
  store i16 %46, ptr %50, !tbaa !9
  %51 = load i16, ptr %3, !tbaa !9
  %52 = sext i16 %51 to i32
  %53 = mul i32 %52, 2
  %54 = getelementptr inbounds i8, ptr @_pz, i32 %53
  %55 = load i16, ptr %54, !tbaa !9
  %56 = load i16, ptr %3, !tbaa !9
  %57 = sext i16 %56 to i32
  %58 = mul i32 %57, 2
  %59 = getelementptr inbounds i8, ptr @_vz, i32 %58
  %60 = load i16, ptr %59, !tbaa !9
  %61 = sext i16 %55 to i32
  %62 = sext i16 %60 to i32
  %63 = add nsw i32 %61, %62
  %64 = trunc i32 %63 to i16
  %65 = load i16, ptr %3, !tbaa !9
  %66 = sext i16 %65 to i32
  %67 = mul i32 %66, 2
  %68 = getelementptr inbounds i8, ptr @_pz, i32 %67
  store i16 %64, ptr %68, !tbaa !9
  %69 = load i16, ptr %3, !tbaa !9
  %70 = sext i16 %69 to i32
  %71 = add nsw i32 %70, 1
  %72 = trunc i32 %71 to i16
  store i16 %72, ptr %3, !tbaa !9
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
