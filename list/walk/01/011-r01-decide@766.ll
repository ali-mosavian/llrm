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

define internal cc 1005 i32 @pipeline.body(ptr noundef %0) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [1024 x i8]
  br label %b2

b2:
  %2 = phi i16 [ 1, %b1 ], [ %17, %b7 ]
  %3 = phi i8 [ 1, %b1 ], [ %18, %b7 ]
  %4 = phi i8 [ 1, %b1 ], [ %19, %b7 ]
  %5 = phi i16 [ 0, %b1 ], [ %20, %b7 ]
  %6 = phi ptr [ @_patterns, %b1 ], [ %21, %b7 ]
  %7 = load i8, ptr %6, !tbaa !7
  switch i8 %7, label %b5 [
    i8 0, label %b3
    i8 124, label %b6
  ]

b3:
  %8 = phi i16 [ %2, %b2 ]
  %9 = icmp slt i16 1, %8
  br i1 %9, label %10, label %b13

10:
  br label %b14

b5:
  %11 = zext i8 %7 to i16
  %12 = getelementptr inbounds nuw i8, ptr @_classof, i16 %11
  %13 = load i8, ptr %12, !tbaa !7
  %14 = icmp eq i8 %13, 0
  br i1 %14, label %b9, label %b8

b6:
  %15 = add i8 %4, 1
  %16 = getelementptr inbounds nuw i8, ptr @_hit, i16 %5
  store i8 %4, ptr %16, !tbaa !7
  br label %b7

b7:
  %17 = phi i16 [ %2, %b6 ], [ %22, %b10 ]
  %18 = phi i8 [ %3, %b6 ], [ %32, %b10 ]
  %19 = phi i8 [ %15, %b6 ], [ %4, %b10 ]
  %20 = phi i16 [ 0, %b6 ], [ %34, %b10 ]
  %21 = getelementptr inbounds nuw i8, ptr %6, i16 1
  br label %b2

b8:
  %22 = phi i16 [ %30, %b9 ], [ %2, %b5 ]
  %23 = shl nuw nsw i16 %5, 5
  %24 = load i8, ptr %12, !tbaa !7
  %25 = zext i8 %24 to i16
  %26 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %23
  %27 = getelementptr inbounds nuw i8, ptr %26, i16 %25
  %28 = load i8, ptr %27, !tbaa !7
  %29 = icmp eq i8 %28, 0
  br i1 %29, label %b11, label %b10

b9:
  %30 = add nsw i16 %2, 1
  %31 = trunc i16 %2 to i8
  store i8 %31, ptr %12, !tbaa !7
  br label %b8

b10:
  %32 = phi i8 [ %35, %b11 ], [ %3, %b8 ]
  %33 = load i8, ptr %27, !tbaa !7
  %34 = zext i8 %33 to i16
  br label %b7

b11:
  %35 = add i8 %3, 1
  store i8 %3, ptr %27, !tbaa !7
  br label %b10

36:
  %split = phi i16 [ %46, %b15 ]
  br label %b13

b13:
  %37 = phi i16 [ %split, %36 ], [ 0, %b3 ]
  %38 = icmp slt i16 0, %37
  %39 = icmp slt i16 0, %8
  %or.cond = and i1 %38, %39
  br i1 %or.cond, label %40, label %b18

40:
  br label %b19

b14:
  %41 = phi i16 [ 1, %10 ], [ %47, %b15 ]
  %42 = phi i16 [ 0, %10 ], [ %46, %b15 ]
  %43 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %41
  %44 = load i8, ptr %43, !tbaa !7
  %45 = icmp eq i8 %44, 0
  br i1 %45, label %b15, label %b16

b15:
  %46 = phi i16 [ %49, %b16 ], [ %42, %b14 ]
  %47 = add nuw nsw i16 %41, 1
  %48 = icmp slt i16 %47, %8
  br i1 %48, label %b14, label %36

b16:
  %49 = add nsw i16 %42, 1
  %50 = getelementptr inbounds i8, ptr @_queue, i16 %42
  store i8 %44, ptr %50, !tbaa !7
  br label %b15

51:
  br label %b18

b18:
  %52 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %53 = icmp sgt i16 %52, 0
  br i1 %53, label %54, label %b29

54:
  br label %b30

b19:
  %55 = phi i16 [ %37, %40 ], [ %split1, %64 ]
  %56 = phi i16 [ 0, %40 ], [ %57, %64 ]
  %57 = add nuw nsw i16 %56, 1
  %58 = getelementptr inbounds nuw i8, ptr @_queue, i16 %56
  %59 = load i8, ptr %58, !tbaa !7
  %60 = zext i8 %59 to i16
  %61 = shl nuw nsw i16 %60, 5
  %62 = getelementptr i8, ptr @_nxt, i16 %61
  %63 = getelementptr inbounds nuw i8, ptr @_fail, i16 %60
  br label %b22

64:
  %split1 = phi i16 [ %87, %b27 ]
  %65 = icmp slt i16 %57, %split1
  br i1 %65, label %b19, label %51

b22:
  %66 = phi i16 [ 0, %b19 ], [ %88, %b27 ]
  %67 = phi i16 [ %55, %b19 ], [ %87, %b27 ]
  %68 = getelementptr i8, ptr %62, i16 %66
  %69 = load i8, ptr %68, !tbaa !7
  %70 = load i8, ptr %63, !tbaa !7
  %71 = zext i8 %70 to i16
  %72 = shl nuw nsw i16 %71, 5
  %73 = getelementptr i8, ptr @_nxt, i16 %72
  %74 = getelementptr i8, ptr %73, i16 %66
  %75 = load i8, ptr %74, !tbaa !7
  %76 = zext i8 %75 to i16
  %77 = icmp eq i8 %69, 0
  br i1 %77, label %b23, label %b24

b23:
  store i8 %75, ptr %68, !tbaa !7
  br label %b27

b24:
  %78 = zext i8 %69 to i16
  %79 = getelementptr inbounds nuw i8, ptr @_fail, i16 %78
  store i8 %75, ptr %79, !tbaa !7
  %80 = getelementptr inbounds nuw i8, ptr @_hit, i16 %78
  %81 = load i8, ptr %80, !tbaa !7
  %82 = icmp eq i8 %81, 0
  br i1 %82, label %b26, label %b25

b25:
  %83 = add nsw i16 %67, 1
  %84 = getelementptr inbounds i8, ptr @_queue, i16 %67
  store i8 %69, ptr %84, !tbaa !7
  br label %b27

b26:
  %85 = getelementptr inbounds nuw i8, ptr @_hit, i16 %76
  %86 = load i8, ptr %85, !tbaa !7
  store i8 %86, ptr %80, !tbaa !7
  br label %b25

b27:
  %87 = phi i16 [ %83, %b25 ], [ %67, %b23 ]
  %88 = add nuw nsw i16 %66, 1
  %89 = icmp slt i16 %88, %8
  br i1 %89, label %b22, label %64

90:
  %split2 = phi i32 [ %103, %b32 ]
  %split3 = phi i32 [ %104, %b32 ]
  %split4 = phi i16 [ %105, %b32 ]
  br label %b29

b29:
  %91 = phi i32 [ %split2, %90 ], [ 0, %b18 ]
  %92 = phi i32 [ %split3, %90 ], [ 0, %b18 ]
  %93 = phi i16 [ %split4, %90 ], [ 0, %b18 ]
  store i32 %91, ptr %0, !tbaa !11
  %94 = zext nneg i16 %93 to i32
  %95 = add nsw i32 %92, %94
  ret i32 %95

b30:
  %96 = phi i16 [ %52, %54 ], [ %107, %b32 ]
  %97 = phi i32 [ 0, %54 ], [ %102, %b32 ]
  %98 = phi i32 [ 0, %54 ], [ %103, %b32 ]
  %99 = phi i32 [ 0, %54 ], [ %104, %b32 ]
  %100 = phi i16 [ 0, %54 ], [ %105, %b32 ]
  %101 = phi i16 [ 0, %54 ], [ %106, %b32 ]
  br label %b33

b32:
  %102 = phi i32 [ %141, %b33 ]
  %103 = phi i32 [ %135, %b33 ]
  %104 = phi i32 [ %140, %b33 ]
  %105 = phi i16 [ %138, %b33 ]
  %106 = phi i16 [ %125, %b33 ]
  %107 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %108 = icmp sgt i16 %107, 0
  br i1 %108, label %b30, label %90

b33:
  %109 = phi i32 [ %97, %b30 ], [ %141, %b33 ]
  %110 = phi i32 [ %98, %b30 ], [ %135, %b33 ]
  %111 = phi i32 [ %99, %b30 ], [ %140, %b33 ]
  %112 = phi i16 [ %100, %b30 ], [ %138, %b33 ]
  %113 = phi i16 [ 0, %b30 ], [ %142, %b33 ]
  %114 = phi i16 [ %101, %b30 ], [ %125, %b33 ]
  %115 = getelementptr inbounds nuw i8, ptr %1, i16 %113
  %116 = load i8, ptr %115, !tbaa !7
  %117 = shl nuw nsw i16 %114, 5
  %118 = zext i8 %116 to i16
  %119 = getelementptr inbounds nuw i8, ptr @_classof, i16 %118
  %120 = load i8, ptr %119, !tbaa !7
  %121 = zext i8 %120 to i16
  %122 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %117
  %123 = getelementptr inbounds nuw i8, ptr %122, i16 %121
  %124 = load i8, ptr %123, !tbaa !7
  %125 = zext i8 %124 to i16
  %126 = getelementptr inbounds nuw i8, ptr @_hit, i16 %125
  %127 = load i8, ptr %126, !tbaa !7
  %128 = icmp eq i8 %127, 0
  %129 = mul nuw nsw i32 %110, 2
  %130 = add nsw i32 %129, %109
  %131 = zext i8 %127 to i32
  %132 = add nsw i32 %130, %131
  %133 = and i32 %132, 536870911
  %134 = select i1 %128, i16 %112, i16 1
  %135 = select i1 %128, i32 %110, i32 %133
  %136 = icmp eq i8 %116, 10
  %137 = zext nneg i16 %134 to i32
  %138 = select i1 %136, i16 0, i16 %134
  %139 = select i1 %136, i32 %137, i32 0
  %140 = add nsw i32 %139, %111
  %141 = add nsw i32 %109, 1
  %142 = add nuw nsw i16 %113, 1
  %143 = icmp slt i16 %142, %96
  br i1 %143, label %b33, label %b32
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
