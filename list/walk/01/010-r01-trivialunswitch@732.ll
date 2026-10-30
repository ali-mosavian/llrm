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
  %split = phi i16 [ %47, %b15 ]
  br label %b13

b13:
  %37 = phi i16 [ %split, %36 ], [ 0, %b3 ]
  %38 = icmp slt i16 0, %37
  br i1 %38, label %39, label %b18

39:
  %40 = icmp slt i16 0, %8
  br i1 %40, label %41, label %53

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
  br label %53

53:
  br label %b18

b18:
  %54 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %55 = icmp sgt i16 %54, 0
  br i1 %55, label %56, label %b29

56:
  br label %b30

b19:
  %57 = phi i16 [ %37, %41 ], [ %split1, %b21 ]
  %58 = phi i16 [ 0, %41 ], [ %59, %b21 ]
  %59 = add nuw nsw i16 %58, 1
  %60 = getelementptr inbounds nuw i8, ptr @_queue, i16 %58
  %61 = load i8, ptr %60, !tbaa !7
  %62 = zext i8 %61 to i16
  br label %63

63:
  %64 = shl nuw nsw i16 %62, 5
  %65 = getelementptr i8, ptr @_nxt, i16 %64
  %66 = getelementptr inbounds nuw i8, ptr @_fail, i16 %62
  br label %b22

67:
  %split1 = phi i16 [ %90, %b27 ]
  br label %b21

b21:
  %68 = icmp slt i16 %59, %split1
  br i1 %68, label %b19, label %52

b22:
  %69 = phi i16 [ 0, %63 ], [ %91, %b27 ]
  %70 = phi i16 [ %57, %63 ], [ %90, %b27 ]
  %71 = getelementptr i8, ptr %65, i16 %69
  %72 = load i8, ptr %71, !tbaa !7
  %73 = load i8, ptr %66, !tbaa !7
  %74 = zext i8 %73 to i16
  %75 = shl nuw nsw i16 %74, 5
  %76 = getelementptr i8, ptr @_nxt, i16 %75
  %77 = getelementptr i8, ptr %76, i16 %69
  %78 = load i8, ptr %77, !tbaa !7
  %79 = zext i8 %78 to i16
  %80 = icmp eq i8 %72, 0
  br i1 %80, label %b23, label %b24

b23:
  store i8 %78, ptr %71, !tbaa !7
  br label %b27

b24:
  %81 = zext i8 %72 to i16
  %82 = getelementptr inbounds nuw i8, ptr @_fail, i16 %81
  store i8 %78, ptr %82, !tbaa !7
  %83 = getelementptr inbounds nuw i8, ptr @_hit, i16 %81
  %84 = load i8, ptr %83, !tbaa !7
  %85 = icmp eq i8 %84, 0
  br i1 %85, label %b26, label %b25

b25:
  %86 = add nsw i16 %70, 1
  %87 = getelementptr inbounds i8, ptr @_queue, i16 %70
  store i8 %72, ptr %87, !tbaa !7
  br label %b27

b26:
  %88 = getelementptr inbounds nuw i8, ptr @_hit, i16 %79
  %89 = load i8, ptr %88, !tbaa !7
  store i8 %89, ptr %83, !tbaa !7
  br label %b25

b27:
  %90 = phi i16 [ %86, %b25 ], [ %70, %b23 ]
  %91 = add nuw nsw i16 %69, 1
  %92 = icmp slt i16 %91, %8
  br i1 %92, label %b22, label %67

93:
  %split2 = phi i32 [ %106, %b32 ]
  %split3 = phi i32 [ %107, %b32 ]
  %split4 = phi i16 [ %108, %b32 ]
  br label %b29

b29:
  %94 = phi i32 [ %split2, %93 ], [ 0, %b18 ]
  %95 = phi i32 [ %split3, %93 ], [ 0, %b18 ]
  %96 = phi i16 [ %split4, %93 ], [ 0, %b18 ]
  store i32 %94, ptr %0, !tbaa !11
  %97 = zext nneg i16 %96 to i32
  %98 = add nsw i32 %95, %97
  ret i32 %98

b30:
  %99 = phi i16 [ %54, %56 ], [ %110, %b32 ]
  %100 = phi i32 [ 0, %56 ], [ %105, %b32 ]
  %101 = phi i32 [ 0, %56 ], [ %106, %b32 ]
  %102 = phi i32 [ 0, %56 ], [ %107, %b32 ]
  %103 = phi i16 [ 0, %56 ], [ %108, %b32 ]
  %104 = phi i16 [ 0, %56 ], [ %109, %b32 ]
  br label %b33

b32:
  %105 = phi i32 [ %144, %b33 ]
  %106 = phi i32 [ %138, %b33 ]
  %107 = phi i32 [ %143, %b33 ]
  %108 = phi i16 [ %141, %b33 ]
  %109 = phi i16 [ %128, %b33 ]
  %110 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %111 = icmp sgt i16 %110, 0
  br i1 %111, label %b30, label %93

b33:
  %112 = phi i32 [ %100, %b30 ], [ %144, %b33 ]
  %113 = phi i32 [ %101, %b30 ], [ %138, %b33 ]
  %114 = phi i32 [ %102, %b30 ], [ %143, %b33 ]
  %115 = phi i16 [ %103, %b30 ], [ %141, %b33 ]
  %116 = phi i16 [ 0, %b30 ], [ %145, %b33 ]
  %117 = phi i16 [ %104, %b30 ], [ %128, %b33 ]
  %118 = getelementptr inbounds nuw i8, ptr %1, i16 %116
  %119 = load i8, ptr %118, !tbaa !7
  %120 = shl nuw nsw i16 %117, 5
  %121 = zext i8 %119 to i16
  %122 = getelementptr inbounds nuw i8, ptr @_classof, i16 %121
  %123 = load i8, ptr %122, !tbaa !7
  %124 = zext i8 %123 to i16
  %125 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %120
  %126 = getelementptr inbounds nuw i8, ptr %125, i16 %124
  %127 = load i8, ptr %126, !tbaa !7
  %128 = zext i8 %127 to i16
  %129 = getelementptr inbounds nuw i8, ptr @_hit, i16 %128
  %130 = load i8, ptr %129, !tbaa !7
  %131 = icmp eq i8 %130, 0
  %132 = mul nuw nsw i32 %113, 2
  %133 = add nsw i32 %132, %112
  %134 = zext i8 %130 to i32
  %135 = add nsw i32 %133, %134
  %136 = and i32 %135, 536870911
  %137 = select i1 %131, i16 %115, i16 1
  %138 = select i1 %131, i32 %113, i32 %136
  %139 = icmp eq i8 %119, 10
  %140 = zext nneg i16 %137 to i32
  %141 = select i1 %139, i16 0, i16 %137
  %142 = select i1 %139, i32 %140, i32 0
  %143 = add nsw i32 %142, %114
  %144 = add nsw i32 %112, 1
  %145 = add nuw nsw i16 %116, 1
  %146 = icmp slt i16 %145, %99
  br i1 %146, label %b33, label %b32
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
