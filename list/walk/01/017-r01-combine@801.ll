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
  br i1 %8, label %9, label %b13

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

35:
  br label %b13

b13:
  %36 = phi i16 [ %45, %35 ], [ 0, %b3 ]
  %37 = icmp sgt i16 %36, 0
  %38 = icmp sgt i16 %2, 0
  %or.cond = and i1 %37, %38
  br i1 %or.cond, label %39, label %b18

39:
  br label %b19

b14:
  %40 = phi i16 [ 1, %9 ], [ %46, %b15 ]
  %41 = phi i16 [ 0, %9 ], [ %45, %b15 ]
  %42 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %40
  %43 = load i8, ptr %42, !tbaa !7
  %44 = icmp eq i8 %43, 0
  br i1 %44, label %b15, label %b16

b15:
  %45 = phi i16 [ %47, %b16 ], [ %41, %b14 ]
  %46 = add nuw nsw i16 %40, 1
  %exitcond2 = icmp eq i16 %46, %2
  br i1 %exitcond2, label %35, label %b14

b16:
  %47 = add nsw i16 %41, 1
  %48 = getelementptr inbounds i8, ptr @_queue, i16 %41
  store i8 %43, ptr %48, !tbaa !7
  br label %b15

49:
  br label %b18

b18:
  %50 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %51 = icmp sgt i16 %50, 0
  br i1 %51, label %52, label %b29

52:
  br label %b30

b19:
  %53 = phi i16 [ %36, %39 ], [ %88, %63 ]
  %54 = phi i16 [ 0, %39 ], [ %55, %63 ]
  %55 = add nuw nsw i16 %54, 1
  %56 = getelementptr inbounds nuw i8, ptr @_queue, i16 %54
  %57 = load i8, ptr %56, !tbaa !7
  %58 = zext i8 %57 to i16
  %59 = shl nuw nsw i16 %58, 5
  %60 = getelementptr i8, ptr @_nxt, i16 %59
  %61 = getelementptr inbounds nuw i8, ptr @_fail, i16 %58
  %62 = load i8, ptr %61, !tbaa !7
  br label %b22

63:
  %64 = icmp slt i16 %55, %88
  br i1 %64, label %b19, label %49

b22:
  %65 = phi i8 [ %62, %b19 ], [ %87, %b27 ]
  %66 = phi i16 [ 0, %b19 ], [ %89, %b27 ]
  %67 = phi i16 [ %53, %b19 ], [ %88, %b27 ]
  %68 = getelementptr i8, ptr %60, i16 %66
  %69 = load i8, ptr %68, !tbaa !7
  %70 = zext i8 %65 to i16
  %71 = shl nuw nsw i16 %70, 5
  %72 = getelementptr i8, ptr @_nxt, i16 %71
  %73 = getelementptr i8, ptr %72, i16 %66
  %74 = load i8, ptr %73, !tbaa !7
  %75 = zext i8 %74 to i16
  %76 = icmp eq i8 %69, 0
  br i1 %76, label %b23, label %b24

b23:
  store i8 %74, ptr %68, !tbaa !7
  br label %b27

b24:
  %77 = zext i8 %69 to i16
  %78 = getelementptr inbounds nuw i8, ptr @_fail, i16 %77
  store i8 %74, ptr %78, !tbaa !7
  %79 = getelementptr inbounds nuw i8, ptr @_hit, i16 %77
  %80 = load i8, ptr %79, !tbaa !7
  %81 = icmp eq i8 %80, 0
  %82 = load i8, ptr %61, !tbaa !7
  br i1 %81, label %b26, label %b25

b25:
  %83 = add nuw nsw i16 %67, 1
  %84 = getelementptr inbounds nuw i8, ptr @_queue, i16 %67
  store i8 %69, ptr %84, !tbaa !7
  br label %b27

b26:
  %85 = getelementptr inbounds nuw i8, ptr @_hit, i16 %75
  %86 = load i8, ptr %85, !tbaa !7
  store i8 %86, ptr %79, !tbaa !7
  br label %b25

b27:
  %87 = phi i8 [ %82, %b25 ], [ %65, %b23 ]
  %88 = phi i16 [ %83, %b25 ], [ %67, %b23 ]
  %89 = add nuw nsw i16 %66, 1
  %exitcond1 = icmp eq i16 %89, %2
  br i1 %exitcond1, label %63, label %b22

90:
  %91 = zext nneg i16 %134 to i32
  %92 = add nsw i32 %136, %91
  br label %b29

b29:
  %93 = phi i32 [ %131, %90 ], [ 0, %b18 ]
  %94 = phi i32 [ %92, %90 ], [ 0, %b18 ]
  store i32 %93, ptr %0, !tbaa !11
  ret i32 %94

b30:
  %95 = phi i16 [ %50, %52 ], [ %103, %b32 ]
  %96 = phi i32 [ 0, %52 ], [ %102, %b32 ]
  %97 = phi i32 [ 0, %52 ], [ %131, %b32 ]
  %98 = phi i32 [ 0, %52 ], [ %136, %b32 ]
  %99 = phi i16 [ 0, %52 ], [ %134, %b32 ]
  %100 = phi i16 [ 0, %52 ], [ %121, %b32 ]
  %101 = zext nneg i16 %95 to i32
  %102 = add i32 %96, %101
  br label %b33

b32:
  %103 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %104 = icmp sgt i16 %103, 0
  br i1 %104, label %b30, label %90

b33:
  %105 = phi i32 [ %96, %b30 ], [ %137, %b33 ]
  %106 = phi i32 [ %97, %b30 ], [ %131, %b33 ]
  %107 = phi i32 [ %98, %b30 ], [ %136, %b33 ]
  %108 = phi i16 [ %99, %b30 ], [ %134, %b33 ]
  %109 = phi i16 [ 0, %b30 ], [ %138, %b33 ]
  %110 = phi i16 [ %100, %b30 ], [ %121, %b33 ]
  %111 = getelementptr inbounds nuw i8, ptr %1, i16 %109
  %112 = load i8, ptr %111, !tbaa !7
  %113 = shl nuw nsw i16 %110, 5
  %114 = zext i8 %112 to i16
  %115 = getelementptr inbounds nuw i8, ptr @_classof, i16 %114
  %116 = load i8, ptr %115, !tbaa !7
  %117 = zext i8 %116 to i16
  %118 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %113
  %119 = getelementptr inbounds nuw i8, ptr %118, i16 %117
  %120 = load i8, ptr %119, !tbaa !7
  %121 = zext i8 %120 to i16
  %122 = getelementptr inbounds nuw i8, ptr @_hit, i16 %121
  %123 = load i8, ptr %122, !tbaa !7
  %124 = icmp eq i8 %123, 0
  %125 = shl nuw nsw i32 %106, 1
  %126 = add nsw i32 %125, %105
  %127 = zext i8 %123 to i32
  %128 = add nsw i32 %126, %127
  %129 = and i32 %128, 536870911
  %130 = select i1 %124, i16 %108, i16 1
  %131 = select i1 %124, i32 %106, i32 %129
  %132 = icmp eq i8 %112, 10
  %133 = zext nneg i16 %130 to i32
  %134 = select i1 %132, i16 0, i16 %130
  %135 = select i1 %132, i32 %133, i32 0
  %136 = add nsw i32 %135, %107
  %137 = add nsw i32 %105, 1
  %138 = add nuw nsw i16 %109, 1
  %exitcond = icmp eq i16 %138, %95
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
