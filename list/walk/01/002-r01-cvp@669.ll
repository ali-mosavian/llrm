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
  %2 = phi i16 [ 1, %b1 ], [ %15, %b7 ]
  %3 = phi i16 [ 1, %b1 ], [ %16, %b7 ]
  %4 = phi i16 [ 1, %b1 ], [ %17, %b7 ]
  %5 = phi i16 [ 0, %b1 ], [ %18, %b7 ]
  %6 = phi ptr [ @_patterns, %b1 ], [ %19, %b7 ]
  %7 = load i8, ptr %6, !tbaa !7
  switch i8 %7, label %b5 [
    i8 0, label %b3
    i8 124, label %b6
  ]

b3:
  br label %b12

b5:
  %8 = zext i8 %7 to i16
  %9 = getelementptr inbounds nuw i8, ptr @_classof, i16 %8
  %10 = load i8, ptr %9, !tbaa !7
  %11 = icmp eq i8 %10, 0
  br i1 %11, label %b9, label %b8

b6:
  %12 = add nsw i16 %4, 1
  %13 = trunc i16 %4 to i8
  %14 = getelementptr inbounds nuw i8, ptr @_hit, i16 %5
  store i8 %13, ptr %14, !tbaa !7
  br label %b7

b7:
  %15 = phi i16 [ %2, %b6 ], [ %20, %b10 ]
  %16 = phi i16 [ %3, %b6 ], [ %30, %b10 ]
  %17 = phi i16 [ %12, %b6 ], [ %4, %b10 ]
  %18 = phi i16 [ 0, %b6 ], [ %32, %b10 ]
  %19 = getelementptr inbounds nuw i8, ptr %6, i16 1
  br label %b2

b8:
  %20 = phi i16 [ %28, %b9 ], [ %2, %b5 ]
  %21 = shl nuw nsw i16 %5, 5
  %22 = load i8, ptr %9, !tbaa !7
  %23 = zext i8 %22 to i16
  %24 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %21
  %25 = getelementptr inbounds nuw i8, ptr %24, i16 %23
  %26 = load i8, ptr %25, !tbaa !7
  %27 = icmp eq i8 %26, 0
  br i1 %27, label %b11, label %b10

b9:
  %28 = add nsw i16 %2, 1
  %29 = trunc i16 %2 to i8
  store i8 %29, ptr %9, !tbaa !7
  br label %b8

b10:
  %30 = phi i16 [ %33, %b11 ], [ %3, %b8 ]
  %31 = load i8, ptr %25, !tbaa !7
  %32 = zext i8 %31 to i16
  br label %b7

b11:
  %33 = add nsw i16 %3, 1
  %34 = trunc i16 %3 to i8
  store i8 %34, ptr %25, !tbaa !7
  br label %b10

b12:
  %35 = phi i16 [ 0, %b3 ], [ %41, %b15 ]
  %36 = phi i16 [ 1, %b3 ], [ %42, %b15 ]
  %37 = icmp slt i16 %36, %2
  br i1 %37, label %b14, label %b13

b13:
  br label %b17

b14:
  %38 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %36
  %39 = load i8, ptr %38, !tbaa !7
  %40 = icmp eq i8 %39, 0
  br i1 %40, label %b15, label %b16

b15:
  %41 = phi i16 [ %43, %b16 ], [ %35, %b14 ]
  %42 = add nsw i16 %36, 1
  br label %b12

b16:
  %43 = add nsw i16 %35, 1
  %44 = getelementptr inbounds i8, ptr @_queue, i16 %35
  store i8 %39, ptr %44, !tbaa !7
  br label %b15

b17:
  %45 = phi i16 [ 0, %b13 ], [ %48, %b21 ]
  %46 = phi i16 [ %35, %b13 ], [ %52, %b21 ]
  %47 = icmp slt i16 %45, %46
  br i1 %47, label %b19, label %b18

b18:
  br label %b28

b19:
  %48 = add nsw i16 %45, 1
  %49 = getelementptr inbounds nuw i8, ptr @_queue, i16 %45
  %50 = load i8, ptr %49, !tbaa !7
  %51 = zext i8 %50 to i16
  br label %b20

b20:
  %52 = phi i16 [ %46, %b19 ], [ %77, %b27 ]
  %53 = phi i16 [ 0, %b19 ], [ %78, %b27 ]
  %54 = icmp slt i16 %53, %2
  br i1 %54, label %b22, label %b21

b21:
  br label %b17

b22:
  %55 = shl nuw nsw i16 %51, 5
  %56 = getelementptr i8, ptr @_nxt, i16 %55
  %57 = getelementptr i8, ptr %56, i16 %53
  %58 = load i8, ptr %57, !tbaa !7
  %59 = getelementptr inbounds nuw i8, ptr @_fail, i16 %51
  %60 = load i8, ptr %59, !tbaa !7
  %61 = zext i8 %60 to i16
  %62 = shl nuw nsw i16 %61, 5
  %63 = getelementptr i8, ptr @_nxt, i16 %62
  %64 = getelementptr i8, ptr %63, i16 %53
  %65 = load i8, ptr %64, !tbaa !7
  %66 = zext i8 %65 to i16
  %67 = icmp eq i8 %58, 0
  br i1 %67, label %b23, label %b24

b23:
  store i8 %65, ptr %57, !tbaa !7
  br label %b27

b24:
  %68 = zext i8 %58 to i16
  %69 = getelementptr inbounds nuw i8, ptr @_fail, i16 %68
  store i8 %65, ptr %69, !tbaa !7
  %70 = getelementptr inbounds nuw i8, ptr @_hit, i16 %68
  %71 = load i8, ptr %70, !tbaa !7
  %72 = icmp eq i8 %71, 0
  br i1 %72, label %b26, label %b25

b25:
  %73 = add nsw i16 %52, 1
  %74 = getelementptr inbounds i8, ptr @_queue, i16 %52
  store i8 %58, ptr %74, !tbaa !7
  br label %b27

b26:
  %75 = getelementptr inbounds nuw i8, ptr @_hit, i16 %66
  %76 = load i8, ptr %75, !tbaa !7
  store i8 %76, ptr %70, !tbaa !7
  br label %b25

b27:
  %77 = phi i16 [ %73, %b25 ], [ %52, %b23 ]
  %78 = add nsw i16 %53, 1
  br label %b20

b28:
  %79 = phi i16 [ 0, %b18 ], [ %88, %b32 ]
  %80 = phi i16 [ 0, %b18 ], [ %90, %b32 ]
  %81 = phi i32 [ 0, %b18 ], [ %91, %b32 ]
  %82 = phi i32 [ 0, %b18 ], [ %92, %b32 ]
  %83 = phi i32 [ 0, %b18 ], [ %93, %b32 ]
  %84 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %85 = icmp sgt i16 %84, 0
  br i1 %85, label %b30, label %b29

b29:
  store i32 %82, ptr %0, !tbaa !11
  %86 = zext nneg i16 %80 to i32
  %87 = add nsw i32 %81, %86
  ret i32 %87

b30:
  br label %b31

b31:
  %88 = phi i16 [ %79, %b30 ], [ %105, %b33 ]
  %89 = phi i16 [ 0, %b30 ], [ %122, %b33 ]
  %90 = phi i16 [ %80, %b30 ], [ %119, %b33 ]
  %91 = phi i32 [ %81, %b30 ], [ %120, %b33 ]
  %92 = phi i32 [ %82, %b30 ], [ %115, %b33 ]
  %93 = phi i32 [ %83, %b30 ], [ %121, %b33 ]
  %94 = icmp slt i16 %89, %84
  br i1 %94, label %b33, label %b32

b32:
  br label %b28

b33:
  %95 = getelementptr inbounds nuw i8, ptr %1, i16 %89
  %96 = load i8, ptr %95, !tbaa !7
  %97 = shl nuw nsw i16 %88, 5
  %98 = zext i8 %96 to i16
  %99 = getelementptr inbounds nuw i8, ptr @_classof, i16 %98
  %100 = load i8, ptr %99, !tbaa !7
  %101 = zext i8 %100 to i16
  %102 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %97
  %103 = getelementptr inbounds nuw i8, ptr %102, i16 %101
  %104 = load i8, ptr %103, !tbaa !7
  %105 = zext i8 %104 to i16
  %106 = getelementptr inbounds nuw i8, ptr @_hit, i16 %105
  %107 = load i8, ptr %106, !tbaa !7
  %108 = icmp eq i8 %107, 0
  %109 = shl nuw nsw i32 %92, 1
  %110 = add nsw i32 %109, %93
  %111 = zext i8 %107 to i32
  %112 = add nsw i32 %110, %111
  %113 = and i32 %112, 536870911
  %114 = select i1 %108, i16 %90, i16 1
  %115 = select i1 %108, i32 %92, i32 %113
  %116 = icmp eq i8 %96, 10
  %117 = zext nneg i16 %114 to i32
  %118 = add nsw i32 %91, %117
  %119 = select i1 %116, i16 0, i16 %114
  %120 = select i1 %116, i32 %118, i32 %91
  %121 = add nsw i32 %93, 1
  %122 = add nsw i16 %89, 1
  br label %b31
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
