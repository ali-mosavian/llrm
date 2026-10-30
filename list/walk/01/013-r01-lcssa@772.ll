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
  %9 = icmp sgt i16 %8, 1
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
  %37 = phi i16 [ %47, %b15 ]
  br label %b13

b13:
  %38 = phi i16 [ %37, %36 ], [ 0, %b3 ]
  %39 = icmp sgt i16 %38, 0
  %40 = icmp sgt i16 %8, 0
  %or.cond = and i1 %39, %40
  br i1 %or.cond, label %41, label %b18

41:
  br label %b19

b14:
  %42 = phi i16 [ 1, %10 ], [ %48, %b15 ]
  %43 = phi i16 [ 0, %10 ], [ %47, %b15 ]
  %44 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %42
  %45 = load i8, ptr %44, !tbaa !7
  %46 = icmp eq i8 %45, 0
  br i1 %46, label %b15, label %b16

b15:
  %47 = phi i16 [ %50, %b16 ], [ %43, %b14 ]
  %48 = add nuw nsw i16 %42, 1
  %49 = icmp slt i16 %48, %8
  br i1 %49, label %b14, label %36

b16:
  %50 = add nsw i16 %43, 1
  %51 = getelementptr inbounds i8, ptr @_queue, i16 %43
  store i8 %45, ptr %51, !tbaa !7
  br label %b15

52:
  br label %b18

b18:
  %53 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %54 = icmp sgt i16 %53, 0
  br i1 %54, label %55, label %b29

55:
  br label %b30

b19:
  %56 = phi i16 [ %38, %41 ], [ %66, %65 ]
  %57 = phi i16 [ 0, %41 ], [ %58, %65 ]
  %58 = add nuw nsw i16 %57, 1
  %59 = getelementptr inbounds nuw i8, ptr @_queue, i16 %57
  %60 = load i8, ptr %59, !tbaa !7
  %61 = zext i8 %60 to i16
  %62 = shl nuw nsw i16 %61, 5
  %63 = getelementptr i8, ptr @_nxt, i16 %62
  %64 = getelementptr inbounds nuw i8, ptr @_fail, i16 %61
  br label %b22

65:
  %66 = phi i16 [ %89, %b27 ]
  %67 = icmp slt i16 %58, %66
  br i1 %67, label %b19, label %52

b22:
  %68 = phi i16 [ 0, %b19 ], [ %90, %b27 ]
  %69 = phi i16 [ %56, %b19 ], [ %89, %b27 ]
  %70 = getelementptr i8, ptr %63, i16 %68
  %71 = load i8, ptr %70, !tbaa !7
  %72 = load i8, ptr %64, !tbaa !7
  %73 = zext i8 %72 to i16
  %74 = shl nuw nsw i16 %73, 5
  %75 = getelementptr i8, ptr @_nxt, i16 %74
  %76 = getelementptr i8, ptr %75, i16 %68
  %77 = load i8, ptr %76, !tbaa !7
  %78 = zext i8 %77 to i16
  %79 = icmp eq i8 %71, 0
  br i1 %79, label %b23, label %b24

b23:
  store i8 %77, ptr %70, !tbaa !7
  br label %b27

b24:
  %80 = zext i8 %71 to i16
  %81 = getelementptr inbounds nuw i8, ptr @_fail, i16 %80
  store i8 %77, ptr %81, !tbaa !7
  %82 = getelementptr inbounds nuw i8, ptr @_hit, i16 %80
  %83 = load i8, ptr %82, !tbaa !7
  %84 = icmp eq i8 %83, 0
  br i1 %84, label %b26, label %b25

b25:
  %85 = add nsw i16 %69, 1
  %86 = getelementptr inbounds i8, ptr @_queue, i16 %69
  store i8 %71, ptr %86, !tbaa !7
  br label %b27

b26:
  %87 = getelementptr inbounds nuw i8, ptr @_hit, i16 %78
  %88 = load i8, ptr %87, !tbaa !7
  store i8 %88, ptr %82, !tbaa !7
  br label %b25

b27:
  %89 = phi i16 [ %85, %b25 ], [ %69, %b23 ]
  %90 = add nuw nsw i16 %68, 1
  %91 = icmp slt i16 %90, %8
  br i1 %91, label %b22, label %65

92:
  %93 = phi i32 [ %108, %b32 ]
  %94 = phi i16 [ %109, %b32 ]
  %95 = phi i32 [ %110, %b32 ]
  %96 = zext nneg i16 %94 to i32
  %97 = add nsw i32 %95, %96
  br label %b29

b29:
  %98 = phi i32 [ %93, %92 ], [ 0, %b18 ]
  %99 = phi i32 [ %97, %92 ], [ 0, %b18 ]
  store i32 %98, ptr %0, !tbaa !11
  ret i32 %99

b30:
  %100 = phi i16 [ %53, %55 ], [ %111, %b32 ]
  %101 = phi i32 [ 0, %55 ], [ %107, %b32 ]
  %102 = phi i32 [ 0, %55 ], [ %108, %b32 ]
  %103 = phi i32 [ 0, %55 ], [ %110, %b32 ]
  %104 = phi i16 [ 0, %55 ], [ %109, %b32 ]
  %105 = phi i16 [ 0, %55 ], [ %106, %b32 ]
  br label %b33

b32:
  %106 = phi i16 [ %129, %b33 ]
  %107 = phi i32 [ %145, %b33 ]
  %108 = phi i32 [ %139, %b33 ]
  %109 = phi i16 [ %142, %b33 ]
  %110 = phi i32 [ %144, %b33 ]
  %111 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %112 = icmp sgt i16 %111, 0
  br i1 %112, label %b30, label %92

b33:
  %113 = phi i32 [ %101, %b30 ], [ %145, %b33 ]
  %114 = phi i32 [ %102, %b30 ], [ %139, %b33 ]
  %115 = phi i32 [ %103, %b30 ], [ %144, %b33 ]
  %116 = phi i16 [ %104, %b30 ], [ %142, %b33 ]
  %117 = phi i16 [ 0, %b30 ], [ %146, %b33 ]
  %118 = phi i16 [ %105, %b30 ], [ %129, %b33 ]
  %119 = getelementptr inbounds nuw i8, ptr %1, i16 %117
  %120 = load i8, ptr %119, !tbaa !7
  %121 = shl nuw nsw i16 %118, 5
  %122 = zext i8 %120 to i16
  %123 = getelementptr inbounds nuw i8, ptr @_classof, i16 %122
  %124 = load i8, ptr %123, !tbaa !7
  %125 = zext i8 %124 to i16
  %126 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %121
  %127 = getelementptr inbounds nuw i8, ptr %126, i16 %125
  %128 = load i8, ptr %127, !tbaa !7
  %129 = zext i8 %128 to i16
  %130 = getelementptr inbounds nuw i8, ptr @_hit, i16 %129
  %131 = load i8, ptr %130, !tbaa !7
  %132 = icmp eq i8 %131, 0
  %133 = shl nuw nsw i32 %114, 1
  %134 = add nsw i32 %133, %113
  %135 = zext i8 %131 to i32
  %136 = add nsw i32 %134, %135
  %137 = and i32 %136, 536870911
  %138 = select i1 %132, i16 %116, i16 1
  %139 = select i1 %132, i32 %114, i32 %137
  %140 = icmp eq i8 %120, 10
  %141 = zext nneg i16 %138 to i32
  %142 = select i1 %140, i16 0, i16 %138
  %143 = select i1 %140, i32 %141, i32 0
  %144 = add nsw i32 %143, %115
  %145 = add nsw i32 %113, 1
  %146 = add nuw nsw i16 %117, 1
  %147 = icmp slt i16 %146, %100
  br i1 %147, label %b33, label %b32
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
