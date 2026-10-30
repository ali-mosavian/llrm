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
  br i1 %38, label %39, label %b18

39:
  %40 = icmp slt i16 0, %8
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
  %55 = phi i16 [ %37, %39 ], [ %66, %b21 ]
  %56 = phi i16 [ 0, %39 ], [ %57, %b21 ]
  %57 = add nuw nsw i16 %56, 1
  %58 = getelementptr inbounds nuw i8, ptr @_queue, i16 %56
  %59 = load i8, ptr %58, !tbaa !7
  %60 = zext i8 %59 to i16
  br i1 %40, label %61, label %b21

61:
  %62 = shl nuw nsw i16 %60, 5
  %63 = getelementptr i8, ptr @_nxt, i16 %62
  %64 = getelementptr inbounds nuw i8, ptr @_fail, i16 %60
  br label %b22

65:
  %split1 = phi i16 [ %89, %b27 ]
  br label %b21

b21:
  %66 = phi i16 [ %split1, %65 ], [ %55, %b19 ]
  %67 = icmp slt i16 %57, %66
  br i1 %67, label %b19, label %51

b22:
  %68 = phi i16 [ 0, %61 ], [ %90, %b27 ]
  %69 = phi i16 [ %55, %61 ], [ %89, %b27 ]
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
  %split2 = phi i32 [ %105, %b32 ]
  %split3 = phi i32 [ %106, %b32 ]
  %split4 = phi i16 [ %107, %b32 ]
  br label %b29

b29:
  %93 = phi i32 [ %split2, %92 ], [ 0, %b18 ]
  %94 = phi i32 [ %split3, %92 ], [ 0, %b18 ]
  %95 = phi i16 [ %split4, %92 ], [ 0, %b18 ]
  store i32 %93, ptr %0, !tbaa !11
  %96 = zext nneg i16 %95 to i32
  %97 = add nsw i32 %94, %96
  ret i32 %97

b30:
  %98 = phi i16 [ %52, %54 ], [ %109, %b32 ]
  %99 = phi i32 [ 0, %54 ], [ %104, %b32 ]
  %100 = phi i32 [ 0, %54 ], [ %105, %b32 ]
  %101 = phi i32 [ 0, %54 ], [ %106, %b32 ]
  %102 = phi i16 [ 0, %54 ], [ %107, %b32 ]
  %103 = phi i16 [ 0, %54 ], [ %108, %b32 ]
  br label %b33

b32:
  %104 = phi i32 [ %143, %b33 ]
  %105 = phi i32 [ %137, %b33 ]
  %106 = phi i32 [ %142, %b33 ]
  %107 = phi i16 [ %140, %b33 ]
  %108 = phi i16 [ %127, %b33 ]
  %109 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %110 = icmp sgt i16 %109, 0
  br i1 %110, label %b30, label %92

b33:
  %111 = phi i32 [ %99, %b30 ], [ %143, %b33 ]
  %112 = phi i32 [ %100, %b30 ], [ %137, %b33 ]
  %113 = phi i32 [ %101, %b30 ], [ %142, %b33 ]
  %114 = phi i16 [ %102, %b30 ], [ %140, %b33 ]
  %115 = phi i16 [ 0, %b30 ], [ %144, %b33 ]
  %116 = phi i16 [ %103, %b30 ], [ %127, %b33 ]
  %117 = getelementptr inbounds nuw i8, ptr %1, i16 %115
  %118 = load i8, ptr %117, !tbaa !7
  %119 = shl nuw nsw i16 %116, 5
  %120 = zext i8 %118 to i16
  %121 = getelementptr inbounds nuw i8, ptr @_classof, i16 %120
  %122 = load i8, ptr %121, !tbaa !7
  %123 = zext i8 %122 to i16
  %124 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %119
  %125 = getelementptr inbounds nuw i8, ptr %124, i16 %123
  %126 = load i8, ptr %125, !tbaa !7
  %127 = zext i8 %126 to i16
  %128 = getelementptr inbounds nuw i8, ptr @_hit, i16 %127
  %129 = load i8, ptr %128, !tbaa !7
  %130 = icmp eq i8 %129, 0
  %131 = mul nuw nsw i32 %112, 2
  %132 = add nsw i32 %131, %111
  %133 = zext i8 %129 to i32
  %134 = add nsw i32 %132, %133
  %135 = and i32 %134, 536870911
  %136 = select i1 %130, i16 %114, i16 1
  %137 = select i1 %130, i32 %112, i32 %135
  %138 = icmp eq i8 %118, 10
  %139 = zext nneg i16 %136 to i32
  %140 = select i1 %138, i16 0, i16 %136
  %141 = select i1 %138, i32 %139, i32 0
  %142 = add nsw i32 %141, %113
  %143 = add nsw i32 %111, 1
  %144 = add nuw nsw i16 %115, 1
  %145 = icmp slt i16 %144, %98
  br i1 %145, label %b33, label %b32
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
