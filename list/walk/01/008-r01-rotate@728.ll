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
  %split = phi i16 [ %45, %b15 ]
  br label %b13

b13:
  %37 = phi i16 [ %split, %36 ], [ 0, %b3 ]
  %38 = icmp slt i16 0, %37
  br i1 %38, label %39, label %b18

39:
  br label %b19

b14:
  %40 = phi i16 [ 1, %10 ], [ %46, %b15 ]
  %41 = phi i16 [ 0, %10 ], [ %45, %b15 ]
  %42 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %40
  %43 = load i8, ptr %42, !tbaa !7
  %44 = icmp eq i8 %43, 0
  br i1 %44, label %b15, label %b16

b15:
  %45 = phi i16 [ %48, %b16 ], [ %41, %b14 ]
  %46 = add nuw nsw i16 %40, 1
  %47 = icmp slt i16 %46, %8
  br i1 %47, label %b14, label %36

b16:
  %48 = add nsw i16 %41, 1
  %49 = getelementptr inbounds i8, ptr @_queue, i16 %41
  store i8 %43, ptr %49, !tbaa !7
  br label %b15

50:
  br label %b18

b18:
  %51 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %52 = icmp sgt i16 %51, 0
  br i1 %52, label %53, label %b29

53:
  br label %b30

b19:
  %54 = phi i16 [ %37, %39 ], [ %63, %b21 ]
  %55 = phi i16 [ 0, %39 ], [ %56, %b21 ]
  %56 = add nuw nsw i16 %55, 1
  %57 = getelementptr inbounds nuw i8, ptr @_queue, i16 %55
  %58 = load i8, ptr %57, !tbaa !7
  %59 = zext i8 %58 to i16
  %60 = icmp slt i16 0, %8
  br i1 %60, label %61, label %b21

61:
  br label %b22

62:
  %split1 = phi i16 [ %89, %b27 ]
  br label %b21

b21:
  %63 = phi i16 [ %split1, %62 ], [ %54, %b19 ]
  %64 = icmp slt i16 %56, %63
  br i1 %64, label %b19, label %50

b22:
  %65 = phi i16 [ 0, %61 ], [ %90, %b27 ]
  %66 = phi i16 [ %54, %61 ], [ %89, %b27 ]
  %67 = shl nuw nsw i16 %59, 5
  %68 = getelementptr i8, ptr @_nxt, i16 %67
  %69 = getelementptr i8, ptr %68, i16 %65
  %70 = load i8, ptr %69, !tbaa !7
  %71 = getelementptr inbounds nuw i8, ptr @_fail, i16 %59
  %72 = load i8, ptr %71, !tbaa !7
  %73 = zext i8 %72 to i16
  %74 = shl nuw nsw i16 %73, 5
  %75 = getelementptr i8, ptr @_nxt, i16 %74
  %76 = getelementptr i8, ptr %75, i16 %65
  %77 = load i8, ptr %76, !tbaa !7
  %78 = zext i8 %77 to i16
  %79 = icmp eq i8 %70, 0
  br i1 %79, label %b23, label %b24

b23:
  store i8 %77, ptr %69, !tbaa !7
  br label %b27

b24:
  %80 = zext i8 %70 to i16
  %81 = getelementptr inbounds nuw i8, ptr @_fail, i16 %80
  store i8 %77, ptr %81, !tbaa !7
  %82 = getelementptr inbounds nuw i8, ptr @_hit, i16 %80
  %83 = load i8, ptr %82, !tbaa !7
  %84 = icmp eq i8 %83, 0
  br i1 %84, label %b26, label %b25

b25:
  %85 = add nsw i16 %66, 1
  %86 = getelementptr inbounds i8, ptr @_queue, i16 %66
  store i8 %70, ptr %86, !tbaa !7
  br label %b27

b26:
  %87 = getelementptr inbounds nuw i8, ptr @_hit, i16 %78
  %88 = load i8, ptr %87, !tbaa !7
  store i8 %88, ptr %82, !tbaa !7
  br label %b25

b27:
  %89 = phi i16 [ %85, %b25 ], [ %66, %b23 ]
  %90 = add nuw nsw i16 %65, 1
  %91 = icmp slt i16 %90, %8
  br i1 %91, label %b22, label %62

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
  %98 = phi i16 [ %51, %53 ], [ %109, %b32 ]
  %99 = phi i32 [ 0, %53 ], [ %104, %b32 ]
  %100 = phi i32 [ 0, %53 ], [ %105, %b32 ]
  %101 = phi i32 [ 0, %53 ], [ %106, %b32 ]
  %102 = phi i16 [ 0, %53 ], [ %107, %b32 ]
  %103 = phi i16 [ 0, %53 ], [ %108, %b32 ]
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
