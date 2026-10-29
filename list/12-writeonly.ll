target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@_patterns = internal constant [81 x i8] c"Oliver|Scrooge|Copperfield|Dombey|Marley|Micawber|Heep|ghost|gentleman|Tiny Tim|\00"
@_queue = internal global [80 x i8] zeroinitializer
@_nxt = internal global [2560 x i8] zeroinitializer
@_fail = internal global [80 x i8] zeroinitializer
@_classof = internal global [256 x i8] zeroinitializer
@_hit = internal global [80 x i8] zeroinitializer

define cc 1005 i32 @"_bench_grep@3"(ptr noundef nocapture writeonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [1024 x i8]
  br label %b2

b2:
  %2 = phi i16 [ 1, %b1 ], [ %15, %b7 ]
  %3 = phi i8 [ 1, %b1 ], [ %16, %b7 ]
  %4 = phi i8 [ 1, %b1 ], [ %17, %b7 ]
  %5 = phi i16 [ 0, %b1 ], [ %18, %b7 ]
  %6 = phi ptr [ @_patterns, %b1 ], [ %19, %b7 ]
  %7 = load i8, ptr %6, !tbaa !7
  switch i8 %7, label %b5 [
    i8 0, label %b3
    i8 124, label %b6
  ]

b3:
  %8 = icmp sgt i16 %2, 1
  br i1 %8, label %b14, label %b18

b5:
  %9 = zext i8 %7 to i16
  %10 = getelementptr inbounds nuw i8, ptr @_classof, i16 %9
  %11 = load i8, ptr %10, !tbaa !7
  %12 = icmp eq i8 %11, 0
  br i1 %12, label %b9, label %b8

b6:
  %13 = add i8 %4, 1
  %14 = getelementptr inbounds nuw i8, ptr @_hit, i16 %5
  store i8 %4, ptr %14, !tbaa !7
  br label %b7

b7:
  %15 = phi i16 [ %2, %b6 ], [ %21, %b10 ]
  %16 = phi i8 [ %3, %b6 ], [ %31, %b10 ]
  %17 = phi i8 [ %13, %b6 ], [ %4, %b10 ]
  %18 = phi i16 [ 0, %b6 ], [ %32, %b10 ]
  %19 = getelementptr inbounds nuw i8, ptr %6, i16 1
  br label %b2

b8:
  %20 = phi i8 [ %29, %b9 ], [ %11, %b5 ]
  %21 = phi i16 [ %28, %b9 ], [ %2, %b5 ]
  %22 = shl nuw nsw i16 %5, 5
  %23 = zext i8 %20 to i16
  %24 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %22
  %25 = getelementptr inbounds nuw i8, ptr %24, i16 %23
  %26 = load i8, ptr %25, !tbaa !7
  %27 = icmp eq i8 %26, 0
  br i1 %27, label %b11, label %b10

b9:
  %28 = add nsw i16 %2, 1
  %29 = trunc i16 %2 to i8
  store i8 %29, ptr %10, !tbaa !7
  br label %b8

b10:
  %30 = phi i8 [ %3, %b11 ], [ %26, %b8 ]
  %31 = phi i8 [ %33, %b11 ], [ %3, %b8 ]
  %32 = zext i8 %30 to i16
  br label %b7

b11:
  %33 = add i8 %3, 1
  store i8 %3, ptr %25, !tbaa !7
  br label %b10

b13:
  %34 = icmp sgt i16 %40, 0
  br i1 %34, label %b19, label %b18

b14:
  %35 = phi i16 [ %41, %b15 ], [ 1, %b3 ]
  %36 = phi i16 [ %40, %b15 ], [ 0, %b3 ]
  %37 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %35
  %38 = load i8, ptr %37, !tbaa !7
  %39 = icmp eq i8 %38, 0
  br i1 %39, label %b15, label %b16

b15:
  %40 = phi i16 [ %42, %b16 ], [ %36, %b14 ]
  %41 = add nuw nsw i16 %35, 1
  %exitcond2 = icmp eq i16 %41, %2
  br i1 %exitcond2, label %b13, label %b14

b16:
  %42 = add nsw i16 %36, 1
  %43 = getelementptr inbounds i8, ptr @_queue, i16 %36
  store i8 %38, ptr %43, !tbaa !7
  br label %b15

b18:
  %44 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %45 = icmp sgt i16 %44, 0
  br i1 %45, label %b30, label %b29

b19:
  %46 = phi i16 [ %81, %56 ], [ %40, %b13 ]
  %47 = phi i16 [ %48, %56 ], [ 0, %b13 ]
  %48 = add nuw nsw i16 %47, 1
  %49 = getelementptr inbounds nuw i8, ptr @_queue, i16 %47
  %50 = load i8, ptr %49, !tbaa !7
  %51 = zext i8 %50 to i16
  %52 = shl nuw nsw i16 %51, 5
  %53 = getelementptr i8, ptr @_nxt, i16 %52
  %54 = getelementptr inbounds nuw i8, ptr @_fail, i16 %51
  %55 = load i8, ptr %54, !tbaa !7
  br label %b22

56:
  %57 = icmp slt i16 %48, %81
  br i1 %57, label %b19, label %b18

b22:
  %58 = phi i8 [ %55, %b19 ], [ %80, %b27 ]
  %59 = phi i16 [ 0, %b19 ], [ %82, %b27 ]
  %60 = phi i16 [ %46, %b19 ], [ %81, %b27 ]
  %61 = getelementptr i8, ptr %53, i16 %59
  %62 = load i8, ptr %61, !tbaa !7
  %63 = zext i8 %58 to i16
  %64 = shl nuw nsw i16 %63, 5
  %65 = getelementptr i8, ptr @_nxt, i16 %64
  %66 = getelementptr i8, ptr %65, i16 %59
  %67 = load i8, ptr %66, !tbaa !7
  %68 = zext i8 %67 to i16
  %69 = icmp eq i8 %62, 0
  br i1 %69, label %b23, label %b24

b23:
  store i8 %67, ptr %61, !tbaa !7
  br label %b27

b24:
  %70 = zext i8 %62 to i16
  %71 = getelementptr inbounds nuw i8, ptr @_fail, i16 %70
  store i8 %67, ptr %71, !tbaa !7
  %72 = getelementptr inbounds nuw i8, ptr @_hit, i16 %70
  %73 = load i8, ptr %72, !tbaa !7
  %74 = icmp eq i8 %73, 0
  %75 = load i8, ptr %54, !tbaa !7
  br i1 %74, label %b26, label %b25

b25:
  %76 = add nuw nsw i16 %60, 1
  %77 = getelementptr inbounds nuw i8, ptr @_queue, i16 %60
  store i8 %62, ptr %77, !tbaa !7
  br label %b27

b26:
  %78 = getelementptr inbounds nuw i8, ptr @_hit, i16 %68
  %79 = load i8, ptr %78, !tbaa !7
  store i8 %79, ptr %72, !tbaa !7
  br label %b25

b27:
  %80 = phi i8 [ %75, %b25 ], [ %58, %b23 ]
  %81 = phi i16 [ %76, %b25 ], [ %60, %b23 ]
  %82 = add nuw nsw i16 %59, 1
  %exitcond1 = icmp eq i16 %82, %2
  br i1 %exitcond1, label %56, label %b22

83:
  %84 = zext nneg i16 %127 to i32
  %85 = add nsw i32 %129, %84
  br label %b29

b29:
  %86 = phi i32 [ %124, %83 ], [ 0, %b18 ]
  %87 = phi i32 [ %85, %83 ], [ 0, %b18 ]
  store i32 %86, ptr %0, !tbaa !11
  ret i32 %87

b30:
  %88 = phi i16 [ %96, %b32 ], [ %44, %b18 ]
  %89 = phi i32 [ %95, %b32 ], [ 0, %b18 ]
  %90 = phi i32 [ %124, %b32 ], [ 0, %b18 ]
  %91 = phi i32 [ %129, %b32 ], [ 0, %b18 ]
  %92 = phi i16 [ %127, %b32 ], [ 0, %b18 ]
  %93 = phi i16 [ %114, %b32 ], [ 0, %b18 ]
  %94 = zext nneg i16 %88 to i32
  %95 = add i32 %89, %94
  br label %b33

b32:
  %96 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %97 = icmp sgt i16 %96, 0
  br i1 %97, label %b30, label %83

b33:
  %98 = phi i32 [ %89, %b30 ], [ %130, %b33 ]
  %99 = phi i32 [ %90, %b30 ], [ %124, %b33 ]
  %100 = phi i32 [ %91, %b30 ], [ %129, %b33 ]
  %101 = phi i16 [ %92, %b30 ], [ %127, %b33 ]
  %102 = phi i16 [ 0, %b30 ], [ %131, %b33 ]
  %103 = phi i16 [ %93, %b30 ], [ %114, %b33 ]
  %104 = getelementptr inbounds nuw i8, ptr %1, i16 %102
  %105 = load i8, ptr %104, !tbaa !7
  %106 = shl nuw nsw i16 %103, 5
  %107 = zext i8 %105 to i16
  %108 = getelementptr inbounds nuw i8, ptr @_classof, i16 %107
  %109 = load i8, ptr %108, !tbaa !7
  %110 = zext i8 %109 to i16
  %111 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %106
  %112 = getelementptr inbounds nuw i8, ptr %111, i16 %110
  %113 = load i8, ptr %112, !tbaa !7
  %114 = zext i8 %113 to i16
  %115 = getelementptr inbounds nuw i8, ptr @_hit, i16 %114
  %116 = load i8, ptr %115, !tbaa !7
  %117 = icmp eq i8 %116, 0
  %118 = shl nuw nsw i32 %99, 1
  %119 = add nsw i32 %118, %98
  %120 = zext i8 %116 to i32
  %121 = add nsw i32 %119, %120
  %122 = and i32 %121, 536870911
  %123 = select i1 %117, i16 %101, i16 1
  %124 = select i1 %117, i32 %99, i32 %122
  %125 = icmp eq i8 %105, 10
  %126 = zext nneg i16 %123 to i32
  %127 = select i1 %125, i16 0, i16 %123
  %128 = select i1 %125, i32 %126, i32 0
  %129 = add nsw i32 %128, %100
  %130 = add nsw i32 %98, 1
  %131 = add nuw nsw i16 %102, 1
  %exitcond = icmp eq i16 %131, %88
  br i1 %exitcond, label %b32, label %b33
}

define i16 @_main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i32
  %1 = call cc 1005 addrspace(1) i32 @"_bench_grep@3"(ptr noundef nonnull %0)
  %2 = tail call cc 1005 addrspace(1) i16 @"_report@3"(i32 noundef %1)
  %3 = load i32, ptr %0, !tbaa !11
  %4 = tail call cc 1005 addrspace(1) i16 @"_report@3"(i32 noundef %3)
  ret i16 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare cc 1005 i16 @"_input_read@3"(ptr, i16) addrspace(1)

declare cc 1005 i16 @"_report@3"(i32) addrspace(1)

declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

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
