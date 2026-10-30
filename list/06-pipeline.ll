target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@_patterns = internal constant [81 x i8] c"Oliver|Scrooge|Copperfield|Dombey|Marley|Micawber|Heep|ghost|gentleman|Tiny Tim|\00"
@_queue = internal global [80 x i8] zeroinitializer
@_nxt = internal global [2560 x i8] zeroinitializer
@_fail = internal global [80 x i8] zeroinitializer
@_classof = internal global [256 x i8] zeroinitializer
@_hit = internal global [80 x i8] zeroinitializer

define cc 1005 i32 @"_bench_grep@3"(ptr noundef %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
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
  %18 = phi i16 [ 0, %b6 ], [ %36, %b10 ]
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
  %30 = phi i16 [ %37, %b11 ], [ %3, %b8 ]
  %31 = load i8, ptr %9, !tbaa !7
  %32 = zext i8 %31 to i16
  %33 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %21
  %34 = getelementptr inbounds nuw i8, ptr %33, i16 %32
  %35 = load i8, ptr %34, !tbaa !7
  %36 = zext i8 %35 to i16
  br label %b7

b11:
  %37 = add nsw i16 %3, 1
  %38 = trunc i16 %3 to i8
  store i8 %38, ptr %25, !tbaa !7
  br label %b10

b12:
  %39 = phi i16 [ 0, %b3 ], [ %45, %b15 ]
  %40 = phi i16 [ 1, %b3 ], [ %46, %b15 ]
  %41 = icmp slt i16 %40, %2
  br i1 %41, label %b14, label %b13

b13:
  br label %b17

b14:
  %42 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %40
  %43 = load i8, ptr %42, !tbaa !7
  %44 = icmp eq i8 %43, 0
  br i1 %44, label %b15, label %b16

b15:
  %45 = phi i16 [ %47, %b16 ], [ %39, %b14 ]
  %46 = add nsw i16 %40, 1
  br label %b12

b16:
  %47 = add nsw i16 %39, 1
  %48 = getelementptr inbounds i8, ptr @_queue, i16 %39
  store i8 %43, ptr %48, !tbaa !7
  br label %b15

b17:
  %49 = phi i16 [ 0, %b13 ], [ %52, %b21 ]
  %50 = phi i16 [ %39, %b13 ], [ %56, %b21 ]
  %51 = icmp slt i16 %49, %50
  br i1 %51, label %b19, label %b18

b18:
  br label %b28

b19:
  %52 = add nsw i16 %49, 1
  %53 = getelementptr inbounds nuw i8, ptr @_queue, i16 %49
  %54 = load i8, ptr %53, !tbaa !7
  %55 = zext i8 %54 to i16
  br label %b20

b20:
  %56 = phi i16 [ %50, %b19 ], [ %81, %b27 ]
  %57 = phi i16 [ 0, %b19 ], [ %82, %b27 ]
  %58 = icmp slt i16 %57, %2
  br i1 %58, label %b22, label %b21

b21:
  br label %b17

b22:
  %59 = shl nuw nsw i16 %55, 5
  %60 = getelementptr i8, ptr @_nxt, i16 %59
  %61 = getelementptr i8, ptr %60, i16 %57
  %62 = load i8, ptr %61, !tbaa !7
  %63 = getelementptr inbounds nuw i8, ptr @_fail, i16 %55
  %64 = load i8, ptr %63, !tbaa !7
  %65 = zext i8 %64 to i16
  %66 = shl nuw nsw i16 %65, 5
  %67 = getelementptr i8, ptr @_nxt, i16 %66
  %68 = getelementptr i8, ptr %67, i16 %57
  %69 = load i8, ptr %68, !tbaa !7
  %70 = zext i8 %69 to i16
  %71 = icmp eq i8 %62, 0
  br i1 %71, label %b23, label %b24

b23:
  store i8 %69, ptr %61, !tbaa !7
  br label %b27

b24:
  %72 = zext i8 %62 to i16
  %73 = getelementptr inbounds nuw i8, ptr @_fail, i16 %72
  store i8 %69, ptr %73, !tbaa !7
  %74 = getelementptr inbounds nuw i8, ptr @_hit, i16 %72
  %75 = load i8, ptr %74, !tbaa !7
  %76 = icmp eq i8 %75, 0
  br i1 %76, label %b26, label %b25

b25:
  %77 = add nsw i16 %56, 1
  %78 = getelementptr inbounds i8, ptr @_queue, i16 %56
  store i8 %62, ptr %78, !tbaa !7
  br label %b27

b26:
  %79 = getelementptr inbounds nuw i8, ptr @_hit, i16 %70
  %80 = load i8, ptr %79, !tbaa !7
  store i8 %80, ptr %74, !tbaa !7
  br label %b25

b27:
  %81 = phi i16 [ %77, %b25 ], [ %56, %b23 ]
  %82 = add nsw i16 %57, 1
  br label %b20

b28:
  %83 = phi i16 [ 0, %b18 ], [ %92, %b32 ]
  %84 = phi i16 [ 0, %b18 ], [ %94, %b32 ]
  %85 = phi i32 [ 0, %b18 ], [ %95, %b32 ]
  %86 = phi i32 [ 0, %b18 ], [ %96, %b32 ]
  %87 = phi i32 [ 0, %b18 ], [ %97, %b32 ]
  %88 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef %1, i16 noundef 1024)
  %89 = icmp sgt i16 %88, 0
  br i1 %89, label %b30, label %b29

b29:
  store i32 %86, ptr %0, !tbaa !11
  %90 = zext nneg i16 %84 to i32
  %91 = add nsw i32 %85, %90
  ret i32 %91

b30:
  br label %b31

b31:
  %92 = phi i16 [ %83, %b30 ], [ %109, %b33 ]
  %93 = phi i16 [ 0, %b30 ], [ %126, %b33 ]
  %94 = phi i16 [ %84, %b30 ], [ %123, %b33 ]
  %95 = phi i32 [ %85, %b30 ], [ %124, %b33 ]
  %96 = phi i32 [ %86, %b30 ], [ %119, %b33 ]
  %97 = phi i32 [ %87, %b30 ], [ %125, %b33 ]
  %98 = icmp slt i16 %93, %88
  br i1 %98, label %b33, label %b32

b32:
  br label %b28

b33:
  %99 = getelementptr inbounds nuw i8, ptr %1, i16 %93
  %100 = load i8, ptr %99, !tbaa !7
  %101 = shl nuw nsw i16 %92, 5
  %102 = zext i8 %100 to i16
  %103 = getelementptr inbounds nuw i8, ptr @_classof, i16 %102
  %104 = load i8, ptr %103, !tbaa !7
  %105 = zext i8 %104 to i16
  %106 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %101
  %107 = getelementptr inbounds nuw i8, ptr %106, i16 %105
  %108 = load i8, ptr %107, !tbaa !7
  %109 = zext i8 %108 to i16
  %110 = getelementptr inbounds nuw i8, ptr @_hit, i16 %109
  %111 = load i8, ptr %110, !tbaa !7
  %112 = icmp eq i8 %111, 0
  %113 = shl nuw nsw i32 %96, 1
  %114 = add nsw i32 %113, %97
  %115 = zext i8 %111 to i32
  %116 = add nsw i32 %114, %115
  %117 = and i32 %116, 536870911
  %118 = select i1 %112, i16 %94, i16 1
  %119 = select i1 %112, i32 %96, i32 %117
  %120 = icmp eq i8 %100, 10
  %121 = zext nneg i16 %118 to i32
  %122 = add nsw i32 %95, %121
  %123 = select i1 %120, i16 0, i16 %118
  %124 = select i1 %120, i32 %122, i32 %95
  %125 = add nsw i32 %97, 1
  %126 = add nsw i16 %93, 1
  br label %b31
}

define i16 @_main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i32
  %1 = call cc 1005 addrspace(1) i32 @"_bench_grep@3"(ptr noundef %0)
  %2 = call cc 1005 addrspace(1) i16 @"_report@3"(i32 noundef %1)
  %3 = load i32, ptr %0, !tbaa !11
  %4 = call cc 1005 addrspace(1) i16 @"_report@3"(i32 noundef %3)
  ret i16 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare cc 1005 i16 @"_input_read@3"(ptr, i16) addrspace(1)

declare cc 1005 i16 @"_report@3"(i32) addrspace(1)

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
