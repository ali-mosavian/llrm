@_patterns = internal constant [81 x i8] c"Oliver|Scrooge|Copperfield|Dombey|Marley|Micawber|Heep|ghost|gentleman|Tiny Tim|\00"
@_queue = internal global [80 x i8] zeroinitializer
@_nxt = internal global [2560 x i8] zeroinitializer
@_fail = internal global [80 x i8] zeroinitializer
@_classof = internal global [256 x i8] zeroinitializer
@_hit = internal global [80 x i8] zeroinitializer

declare cc 1005 i32 @"_bench_grep@3"(ptr noundef) addrspace(1) "llrm-fp-eval"="x87-extended"

declare i16 @_main() addrspace(1) "llrm-fp-eval"="x87-extended"

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare cc 1005 i16 @"_input_read@3"(ptr, i16) addrspace(1)

declare cc 1005 i16 @"_report@3"(i32) addrspace(1)

declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

define internal cc 1005 i32 @pipeline.body(ptr noundef %0) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [1024 x i8]
  br label %b2

b2:
  %2 = phi i16 [ 1, %b1 ], [ %16, %b7 ]
  %3 = phi i8 [ 1, %b1 ], [ %17, %b7 ]
  %4 = phi i8 [ 1, %b1 ], [ %18, %b7 ]
  %5 = phi i16 [ 0, %b1 ], [ %19, %b7 ]
  %6 = phi ptr [ @_patterns, %b1 ], [ %20, %b7 ]
  %7 = load i8, ptr %6, !tbaa !7
  switch i8 %7, label %b5 [
    i8 0, label %b3
    i8 124, label %b6
  ]

b3:
  %8 = icmp sgt i16 %2, 1
  br i1 %8, label %9, label %b18

9:
  br label %b14

b5:
  %10 = zext i8 %7 to i16
  %11 = getelementptr inbounds nuw i8, ptr @_classof, i16 %10
  %12 = load i8, ptr %11, !tbaa !7
  %13 = icmp eq i8 %12, 0
  br i1 %13, label %b9, label %b8

b6:
  %14 = add i8 %4, 1
  %15 = getelementptr inbounds nuw i8, ptr @_hit, i16 %5
  store i8 %4, ptr %15, !tbaa !7
  br label %b7

b7:
  %16 = phi i16 [ %2, %b6 ], [ %22, %b10 ]
  %17 = phi i8 [ %3, %b6 ], [ %32, %b10 ]
  %18 = phi i8 [ %14, %b6 ], [ %4, %b10 ]
  %19 = phi i16 [ 0, %b6 ], [ %33, %b10 ]
  %20 = getelementptr inbounds nuw i8, ptr %6, i16 1
  br label %b2

b8:
  %21 = phi i8 [ %30, %b9 ], [ %12, %b5 ]
  %22 = phi i16 [ %29, %b9 ], [ %2, %b5 ]
  %23 = shl nuw nsw i16 %5, 5
  %24 = zext i8 %21 to i16
  %25 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %23
  %26 = getelementptr inbounds nuw i8, ptr %25, i16 %24
  %27 = load i8, ptr %26, !tbaa !7
  %28 = icmp eq i8 %27, 0
  br i1 %28, label %b11, label %b10

b9:
  %29 = add nsw i16 %2, 1
  %30 = trunc i16 %2 to i8
  store i8 %30, ptr %11, !tbaa !7
  br label %b8

b10:
  %31 = phi i8 [ %3, %b11 ], [ %27, %b8 ]
  %32 = phi i8 [ %34, %b11 ], [ %3, %b8 ]
  %33 = zext i8 %31 to i16
  br label %b7

b11:
  %34 = add i8 %3, 1
  store i8 %3, ptr %26, !tbaa !7
  br label %b10

b13:
  %35 = phi i16 [ %44, %b15 ]
  %36 = icmp sgt i16 %35, 0
  %37 = icmp sgt i16 %2, 0
  %or.cond = and i1 %36, %37
  br i1 %or.cond, label %38, label %b18

38:
  br label %b19

b14:
  %39 = phi i16 [ 1, %9 ], [ %45, %b15 ]
  %40 = phi i16 [ 0, %9 ], [ %44, %b15 ]
  %41 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %39
  %42 = load i8, ptr %41, !tbaa !7
  %43 = icmp eq i8 %42, 0
  br i1 %43, label %b15, label %b16

b15:
  %44 = phi i16 [ %46, %b16 ], [ %40, %b14 ]
  %45 = add nuw nsw i16 %39, 1
  %exitcond2 = icmp eq i16 %45, %2
  br i1 %exitcond2, label %b13, label %b14

b16:
  %46 = add nsw i16 %40, 1
  %47 = getelementptr inbounds i8, ptr @_queue, i16 %40
  store i8 %42, ptr %47, !tbaa !7
  br label %b15

b18:
  %48 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %49 = icmp sgt i16 %48, 0
  br i1 %49, label %50, label %b29

50:
  br label %b30

b19:
  %51 = phi i16 [ %35, %38 ], [ %86, %61 ]
  %52 = phi i16 [ 0, %38 ], [ %53, %61 ]
  %53 = add nuw nsw i16 %52, 1
  %54 = getelementptr inbounds nuw i8, ptr @_queue, i16 %52
  %55 = load i8, ptr %54, !tbaa !7
  %56 = zext i8 %55 to i16
  %57 = shl nuw nsw i16 %56, 5
  %58 = getelementptr i8, ptr @_nxt, i16 %57
  %59 = getelementptr inbounds nuw i8, ptr @_fail, i16 %56
  %60 = load i8, ptr %59, !tbaa !7
  br label %b22

61:
  %62 = icmp slt i16 %53, %86
  br i1 %62, label %b19, label %b18

b22:
  %63 = phi i8 [ %60, %b19 ], [ %85, %b27 ]
  %64 = phi i16 [ 0, %b19 ], [ %87, %b27 ]
  %65 = phi i16 [ %51, %b19 ], [ %86, %b27 ]
  %66 = getelementptr i8, ptr %58, i16 %64
  %67 = load i8, ptr %66, !tbaa !7
  %68 = zext i8 %63 to i16
  %69 = shl nuw nsw i16 %68, 5
  %70 = getelementptr i8, ptr @_nxt, i16 %69
  %71 = getelementptr i8, ptr %70, i16 %64
  %72 = load i8, ptr %71, !tbaa !7
  %73 = zext i8 %72 to i16
  %74 = icmp eq i8 %67, 0
  br i1 %74, label %b23, label %b24

b23:
  store i8 %72, ptr %66, !tbaa !7
  br label %b27

b24:
  %75 = zext i8 %67 to i16
  %76 = getelementptr inbounds nuw i8, ptr @_fail, i16 %75
  store i8 %72, ptr %76, !tbaa !7
  %77 = getelementptr inbounds nuw i8, ptr @_hit, i16 %75
  %78 = load i8, ptr %77, !tbaa !7
  %79 = icmp eq i8 %78, 0
  %80 = load i8, ptr %59, !tbaa !7
  br i1 %79, label %b26, label %b25

b25:
  %81 = add nuw nsw i16 %65, 1
  %82 = getelementptr inbounds nuw i8, ptr @_queue, i16 %65
  store i8 %67, ptr %82, !tbaa !7
  br label %b27

b26:
  %83 = getelementptr inbounds nuw i8, ptr @_hit, i16 %73
  %84 = load i8, ptr %83, !tbaa !7
  store i8 %84, ptr %77, !tbaa !7
  br label %b25

b27:
  %85 = phi i8 [ %80, %b25 ], [ %63, %b23 ]
  %86 = phi i16 [ %81, %b25 ], [ %65, %b23 ]
  %87 = add nuw nsw i16 %64, 1
  %exitcond1 = icmp eq i16 %87, %2
  br i1 %exitcond1, label %61, label %b22

88:
  %89 = zext nneg i16 %132 to i32
  %90 = add nsw i32 %134, %89
  br label %b29

b29:
  %91 = phi i32 [ %129, %88 ], [ 0, %b18 ]
  %92 = phi i32 [ %90, %88 ], [ 0, %b18 ]
  store i32 %91, ptr %0, !tbaa !11
  ret i32 %92

b30:
  %93 = phi i16 [ %48, %50 ], [ %101, %b32 ]
  %94 = phi i32 [ 0, %50 ], [ %100, %b32 ]
  %95 = phi i32 [ 0, %50 ], [ %129, %b32 ]
  %96 = phi i32 [ 0, %50 ], [ %134, %b32 ]
  %97 = phi i16 [ 0, %50 ], [ %132, %b32 ]
  %98 = phi i16 [ 0, %50 ], [ %119, %b32 ]
  %99 = zext nneg i16 %93 to i32
  %100 = add i32 %94, %99
  br label %b33

b32:
  %101 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %102 = icmp sgt i16 %101, 0
  br i1 %102, label %b30, label %88

b33:
  %103 = phi i32 [ %94, %b30 ], [ %135, %b33 ]
  %104 = phi i32 [ %95, %b30 ], [ %129, %b33 ]
  %105 = phi i32 [ %96, %b30 ], [ %134, %b33 ]
  %106 = phi i16 [ %97, %b30 ], [ %132, %b33 ]
  %107 = phi i16 [ 0, %b30 ], [ %136, %b33 ]
  %108 = phi i16 [ %98, %b30 ], [ %119, %b33 ]
  %109 = getelementptr inbounds nuw i8, ptr %1, i16 %107
  %110 = load i8, ptr %109, !tbaa !7
  %111 = shl nuw nsw i16 %108, 5
  %112 = zext i8 %110 to i16
  %113 = getelementptr inbounds nuw i8, ptr @_classof, i16 %112
  %114 = load i8, ptr %113, !tbaa !7
  %115 = zext i8 %114 to i16
  %116 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %111
  %117 = getelementptr inbounds nuw i8, ptr %116, i16 %115
  %118 = load i8, ptr %117, !tbaa !7
  %119 = zext i8 %118 to i16
  %120 = getelementptr inbounds nuw i8, ptr @_hit, i16 %119
  %121 = load i8, ptr %120, !tbaa !7
  %122 = icmp eq i8 %121, 0
  %123 = shl nuw nsw i32 %104, 1
  %124 = add nsw i32 %123, %103
  %125 = zext i8 %121 to i32
  %126 = add nsw i32 %124, %125
  %127 = and i32 %126, 536870911
  %128 = select i1 %122, i16 %106, i16 1
  %129 = select i1 %122, i32 %104, i32 %127
  %130 = icmp eq i8 %110, 10
  %131 = zext nneg i16 %128 to i32
  %132 = select i1 %130, i16 0, i16 %128
  %133 = select i1 %130, i32 %131, i32 0
  %134 = add nsw i32 %133, %105
  %135 = add nsw i32 %103, 1
  %136 = add nuw nsw i16 %107, 1
  %exitcond = icmp eq i16 %136, %93
  br i1 %exitcond, label %b32, label %b33
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
