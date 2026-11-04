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
  br i1 %9, label %10, label %b18

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
  %17 = phi i16 [ %2, %b6 ], [ %23, %b10 ]
  %18 = phi i8 [ %3, %b6 ], [ %33, %b10 ]
  %19 = phi i8 [ %15, %b6 ], [ %4, %b10 ]
  %20 = phi i16 [ 0, %b6 ], [ %34, %b10 ]
  %21 = getelementptr inbounds nuw i8, ptr %6, i16 1
  br label %b2

b8:
  %22 = phi i8 [ %31, %b9 ], [ %13, %b5 ]
  %23 = phi i16 [ %30, %b9 ], [ %2, %b5 ]
  %24 = shl nuw nsw i16 %5, 5
  %25 = zext i8 %22 to i16
  %26 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %24
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
  %32 = phi i8 [ %3, %b11 ], [ %28, %b8 ]
  %33 = phi i8 [ %35, %b11 ], [ %3, %b8 ]
  %34 = zext i8 %32 to i16
  br label %b7

b11:
  %35 = add i8 %3, 1
  store i8 %3, ptr %27, !tbaa !7
  br label %b10

b13:
  %36 = phi i16 [ %44, %b15 ]
  %37 = icmp sgt i16 %36, 0
  br i1 %37, label %38, label %b18

38:
  br label %b19

b14:
  %39 = phi i16 [ %45, %b15 ], [ 1, %10 ]
  %40 = phi i16 [ %44, %b15 ], [ 0, %10 ]
  %41 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %39
  %42 = load i8, ptr %41, !tbaa !7
  %43 = icmp eq i8 %42, 0
  br i1 %43, label %b15, label %b16

b15:
  %44 = phi i16 [ %46, %b16 ], [ %40, %b14 ]
  %45 = add nuw nsw i16 %39, 1
  %exitcond2 = icmp eq i16 %45, %8
  br i1 %exitcond2, label %b13, label %b14

b16:
  %46 = add nsw i16 %40, 1
  %47 = getelementptr inbounds i8, ptr @_queue, i16 %40
  store i8 %42, ptr %47, !tbaa !7
  br label %b15

48:
  br label %b18

b18:
  %49 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %50 = icmp sgt i16 %49, 0
  br i1 %50, label %51, label %b29

51:
  br label %b30

b19:
  %52 = phi i16 [ %63, %62 ], [ %36, %38 ]
  %53 = phi i16 [ %54, %62 ], [ 0, %38 ]
  %54 = add nuw nsw i16 %53, 1
  %55 = getelementptr inbounds nuw i8, ptr @_queue, i16 %53
  %56 = load i8, ptr %55, !tbaa !7
  %57 = zext i8 %56 to i16
  %58 = shl nuw nsw i16 %57, 5
  %59 = getelementptr i8, ptr @_nxt, i16 %58
  %60 = getelementptr inbounds nuw i8, ptr @_fail, i16 %57
  %61 = load i8, ptr %60, !tbaa !7
  br label %b22

62:
  %63 = phi i16 [ %88, %b27 ]
  %64 = icmp slt i16 %54, %63
  br i1 %64, label %b19, label %48

b22:
  %65 = phi i8 [ %61, %b19 ], [ %87, %b27 ]
  %66 = phi i16 [ 0, %b19 ], [ %89, %b27 ]
  %67 = phi i16 [ %52, %b19 ], [ %88, %b27 ]
  %68 = getelementptr i8, ptr %59, i16 %66
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
  %82 = load i8, ptr %60, !tbaa !7
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
  %exitcond1 = icmp eq i16 %89, %8
  br i1 %exitcond1, label %62, label %b22

90:
  %91 = phi i32 [ %107, %b32 ]
  %92 = phi i16 [ %108, %b32 ]
  %93 = phi i32 [ %109, %b32 ]
  %94 = zext nneg i16 %92 to i32
  %95 = add nsw i32 %93, %94
  br label %b29

b29:
  %96 = phi i32 [ %91, %90 ], [ 0, %b18 ]
  %97 = phi i32 [ %95, %90 ], [ 0, %b18 ]
  store i32 %96, ptr %0, !tbaa !11
  ret i32 %97

b30:
  %98 = phi i16 [ %110, %b32 ], [ %49, %51 ]
  %99 = phi i32 [ %105, %b32 ], [ 0, %51 ]
  %100 = phi i32 [ %107, %b32 ], [ 0, %51 ]
  %101 = phi i32 [ %109, %b32 ], [ 0, %51 ]
  %102 = phi i16 [ %108, %b32 ], [ 0, %51 ]
  %103 = phi i16 [ %106, %b32 ], [ 0, %51 ]
  %104 = zext nneg i16 %98 to i32
  %105 = add i32 %99, %104
  br label %b33

b32:
  %106 = phi i16 [ %128, %b33 ]
  %107 = phi i32 [ %138, %b33 ]
  %108 = phi i16 [ %141, %b33 ]
  %109 = phi i32 [ %143, %b33 ]
  %110 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %111 = icmp sgt i16 %110, 0
  br i1 %111, label %b30, label %90

b33:
  %112 = phi i32 [ %99, %b30 ], [ %144, %b33 ]
  %113 = phi i32 [ %100, %b30 ], [ %138, %b33 ]
  %114 = phi i32 [ %101, %b30 ], [ %143, %b33 ]
  %115 = phi i16 [ %102, %b30 ], [ %141, %b33 ]
  %116 = phi i16 [ 0, %b30 ], [ %145, %b33 ]
  %117 = phi i16 [ %103, %b30 ], [ %128, %b33 ]
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
  %132 = shl nuw nsw i32 %113, 1
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
  %exitcond = icmp eq i16 %145, %98
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
