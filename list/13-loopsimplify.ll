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
  %35 = icmp sgt i16 %42, 0
  br i1 %35, label %36, label %b18

36:
  br label %b19

b14:
  %37 = phi i16 [ %43, %b15 ], [ 1, %9 ]
  %38 = phi i16 [ %42, %b15 ], [ 0, %9 ]
  %39 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %37
  %40 = load i8, ptr %39, !tbaa !7
  %41 = icmp eq i8 %40, 0
  br i1 %41, label %b15, label %b16

b15:
  %42 = phi i16 [ %44, %b16 ], [ %38, %b14 ]
  %43 = add nuw nsw i16 %37, 1
  %exitcond2 = icmp eq i16 %43, %2
  br i1 %exitcond2, label %b13, label %b14

b16:
  %44 = add nsw i16 %38, 1
  %45 = getelementptr inbounds i8, ptr @_queue, i16 %38
  store i8 %40, ptr %45, !tbaa !7
  br label %b15

46:
  br label %b18

b18:
  %47 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %48 = icmp sgt i16 %47, 0
  br i1 %48, label %49, label %b29

49:
  br label %b30

b19:
  %50 = phi i16 [ %85, %60 ], [ %42, %36 ]
  %51 = phi i16 [ %52, %60 ], [ 0, %36 ]
  %52 = add nuw nsw i16 %51, 1
  %53 = getelementptr inbounds nuw i8, ptr @_queue, i16 %51
  %54 = load i8, ptr %53, !tbaa !7
  %55 = zext i8 %54 to i16
  %56 = shl nuw nsw i16 %55, 5
  %57 = getelementptr i8, ptr @_nxt, i16 %56
  %58 = getelementptr inbounds nuw i8, ptr @_fail, i16 %55
  %59 = load i8, ptr %58, !tbaa !7
  br label %b22

60:
  %61 = icmp slt i16 %52, %85
  br i1 %61, label %b19, label %46

b22:
  %62 = phi i8 [ %59, %b19 ], [ %84, %b27 ]
  %63 = phi i16 [ 0, %b19 ], [ %86, %b27 ]
  %64 = phi i16 [ %50, %b19 ], [ %85, %b27 ]
  %65 = getelementptr i8, ptr %57, i16 %63
  %66 = load i8, ptr %65, !tbaa !7
  %67 = zext i8 %62 to i16
  %68 = shl nuw nsw i16 %67, 5
  %69 = getelementptr i8, ptr @_nxt, i16 %68
  %70 = getelementptr i8, ptr %69, i16 %63
  %71 = load i8, ptr %70, !tbaa !7
  %72 = zext i8 %71 to i16
  %73 = icmp eq i8 %66, 0
  br i1 %73, label %b23, label %b24

b23:
  store i8 %71, ptr %65, !tbaa !7
  br label %b27

b24:
  %74 = zext i8 %66 to i16
  %75 = getelementptr inbounds nuw i8, ptr @_fail, i16 %74
  store i8 %71, ptr %75, !tbaa !7
  %76 = getelementptr inbounds nuw i8, ptr @_hit, i16 %74
  %77 = load i8, ptr %76, !tbaa !7
  %78 = icmp eq i8 %77, 0
  %79 = load i8, ptr %58, !tbaa !7
  br i1 %78, label %b26, label %b25

b25:
  %80 = add nuw nsw i16 %64, 1
  %81 = getelementptr inbounds nuw i8, ptr @_queue, i16 %64
  store i8 %66, ptr %81, !tbaa !7
  br label %b27

b26:
  %82 = getelementptr inbounds nuw i8, ptr @_hit, i16 %72
  %83 = load i8, ptr %82, !tbaa !7
  store i8 %83, ptr %76, !tbaa !7
  br label %b25

b27:
  %84 = phi i8 [ %79, %b25 ], [ %62, %b23 ]
  %85 = phi i16 [ %80, %b25 ], [ %64, %b23 ]
  %86 = add nuw nsw i16 %63, 1
  %exitcond1 = icmp eq i16 %86, %2
  br i1 %exitcond1, label %60, label %b22

87:
  %88 = zext nneg i16 %131 to i32
  %89 = add nsw i32 %133, %88
  br label %b29

b29:
  %90 = phi i32 [ %128, %87 ], [ 0, %b18 ]
  %91 = phi i32 [ %89, %87 ], [ 0, %b18 ]
  store i32 %90, ptr %0, !tbaa !11
  ret i32 %91

b30:
  %92 = phi i16 [ %100, %b32 ], [ %47, %49 ]
  %93 = phi i32 [ %99, %b32 ], [ 0, %49 ]
  %94 = phi i32 [ %128, %b32 ], [ 0, %49 ]
  %95 = phi i32 [ %133, %b32 ], [ 0, %49 ]
  %96 = phi i16 [ %131, %b32 ], [ 0, %49 ]
  %97 = phi i16 [ %118, %b32 ], [ 0, %49 ]
  %98 = zext nneg i16 %92 to i32
  %99 = add i32 %93, %98
  br label %b33

b32:
  %100 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %101 = icmp sgt i16 %100, 0
  br i1 %101, label %b30, label %87

b33:
  %102 = phi i32 [ %93, %b30 ], [ %134, %b33 ]
  %103 = phi i32 [ %94, %b30 ], [ %128, %b33 ]
  %104 = phi i32 [ %95, %b30 ], [ %133, %b33 ]
  %105 = phi i16 [ %96, %b30 ], [ %131, %b33 ]
  %106 = phi i16 [ 0, %b30 ], [ %135, %b33 ]
  %107 = phi i16 [ %97, %b30 ], [ %118, %b33 ]
  %108 = getelementptr inbounds nuw i8, ptr %1, i16 %106
  %109 = load i8, ptr %108, !tbaa !7
  %110 = shl nuw nsw i16 %107, 5
  %111 = zext i8 %109 to i16
  %112 = getelementptr inbounds nuw i8, ptr @_classof, i16 %111
  %113 = load i8, ptr %112, !tbaa !7
  %114 = zext i8 %113 to i16
  %115 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %110
  %116 = getelementptr inbounds nuw i8, ptr %115, i16 %114
  %117 = load i8, ptr %116, !tbaa !7
  %118 = zext i8 %117 to i16
  %119 = getelementptr inbounds nuw i8, ptr @_hit, i16 %118
  %120 = load i8, ptr %119, !tbaa !7
  %121 = icmp eq i8 %120, 0
  %122 = shl nuw nsw i32 %103, 1
  %123 = add nsw i32 %122, %102
  %124 = zext i8 %120 to i32
  %125 = add nsw i32 %123, %124
  %126 = and i32 %125, 536870911
  %127 = select i1 %121, i16 %105, i16 1
  %128 = select i1 %121, i32 %103, i32 %126
  %129 = icmp eq i8 %109, 10
  %130 = zext nneg i16 %127 to i32
  %131 = select i1 %129, i16 0, i16 %127
  %132 = select i1 %129, i32 %130, i32 0
  %133 = add nsw i32 %132, %104
  %134 = add nsw i32 %102, 1
  %135 = add nuw nsw i16 %106, 1
  %exitcond = icmp eq i16 %135, %92
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
