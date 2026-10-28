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
  %2 = phi i16 [ 1, %b1 ], [ %21, %b7 ]
  %3 = phi i16 [ 1, %b1 ], [ %22, %b7 ]
  %4 = phi i16 [ 1, %b1 ], [ %23, %b7 ]
  %5 = phi i16 [ 0, %b1 ], [ %24, %b7 ]
  %6 = phi ptr [ @_patterns, %b1 ], [ %25, %b7 ]
  %7 = load i8, ptr %6, !tbaa !7
  %8 = sext i8 %7 to i16
  %9 = icmp ne i16 %8, 0
  br i1 %9, label %b4, label %b3

b3:
  br label %b12

b4:
  %10 = load i8, ptr %6, !tbaa !7
  %11 = zext i8 %10 to i16
  %12 = icmp eq i16 %11, 124
  br i1 %12, label %b6, label %b5

b5:
  %13 = zext i8 %10 to i16
  %14 = getelementptr inbounds i8, ptr @_classof, i16 %13
  %15 = load i8, ptr %14, !tbaa !7
  %16 = zext i8 %15 to i16
  %17 = icmp eq i16 %16, 0
  br i1 %17, label %b9, label %b8

b6:
  %18 = add nsw i16 %4, 1
  %19 = trunc i16 %4 to i8
  %20 = getelementptr inbounds i8, ptr @_hit, i16 %5
  store i8 %19, ptr %20, !tbaa !7
  br label %b7

b7:
  %21 = phi i16 [ %2, %b6 ], [ %26, %b10 ]
  %22 = phi i16 [ %3, %b6 ], [ %41, %b10 ]
  %23 = phi i16 [ %18, %b6 ], [ %4, %b10 ]
  %24 = phi i16 [ 0, %b6 ], [ %50, %b10 ]
  %25 = getelementptr inbounds i8, ptr %6, i16 1
  br label %b2

b8:
  %26 = phi i16 [ %37, %b9 ], [ %2, %b5 ]
  %27 = mul nsw i16 %5, 32
  %28 = zext i8 %10 to i16
  %29 = getelementptr inbounds i8, ptr @_classof, i16 %28
  %30 = load i8, ptr %29, !tbaa !7
  %31 = zext i8 %30 to i16
  %32 = add nsw i16 %27, %31
  %33 = getelementptr inbounds i8, ptr @_nxt, i16 %32
  %34 = load i8, ptr %33, !tbaa !7
  %35 = zext i8 %34 to i16
  %36 = icmp eq i16 %35, 0
  br i1 %36, label %b11, label %b10

b9:
  %37 = add nsw i16 %2, 1
  %38 = trunc i16 %2 to i8
  %39 = zext i8 %10 to i16
  %40 = getelementptr inbounds i8, ptr @_classof, i16 %39
  store i8 %38, ptr %40, !tbaa !7
  br label %b8

b10:
  %41 = phi i16 [ %51, %b11 ], [ %3, %b8 ]
  %42 = mul nsw i16 %5, 32
  %43 = zext i8 %10 to i16
  %44 = getelementptr inbounds i8, ptr @_classof, i16 %43
  %45 = load i8, ptr %44, !tbaa !7
  %46 = zext i8 %45 to i16
  %47 = add nsw i16 %42, %46
  %48 = getelementptr inbounds i8, ptr @_nxt, i16 %47
  %49 = load i8, ptr %48, !tbaa !7
  %50 = zext i8 %49 to i16
  br label %b7

b11:
  %51 = add nsw i16 %3, 1
  %52 = trunc i16 %3 to i8
  %53 = mul nsw i16 %5, 32
  %54 = zext i8 %10 to i16
  %55 = getelementptr inbounds i8, ptr @_classof, i16 %54
  %56 = load i8, ptr %55, !tbaa !7
  %57 = zext i8 %56 to i16
  %58 = add nsw i16 %53, %57
  %59 = getelementptr inbounds i8, ptr @_nxt, i16 %58
  store i8 %52, ptr %59, !tbaa !7
  br label %b10

b12:
  %60 = phi i16 [ 0, %b3 ], [ %67, %b15 ]
  %61 = phi i16 [ 1, %b3 ], [ %68, %b15 ]
  %62 = icmp slt i16 %61, %2
  br i1 %62, label %b14, label %b13

b13:
  br label %b17

b14:
  %63 = getelementptr inbounds i8, ptr @_nxt, i16 %61
  %64 = load i8, ptr %63, !tbaa !7
  %65 = zext i8 %64 to i16
  %66 = icmp ne i16 %65, 0
  br i1 %66, label %b16, label %b15

b15:
  %67 = phi i16 [ %71, %b16 ], [ %60, %b14 ]
  %68 = add nsw i16 %61, 1
  br label %b12

b16:
  %69 = getelementptr inbounds i8, ptr @_nxt, i16 %61
  %70 = load i8, ptr %69, !tbaa !7
  %71 = add nsw i16 %60, 1
  %72 = getelementptr inbounds i8, ptr @_queue, i16 %60
  store i8 %70, ptr %72, !tbaa !7
  br label %b15

b17:
  %73 = phi i16 [ 0, %b13 ], [ %76, %b21 ]
  %74 = phi i16 [ %60, %b13 ], [ %80, %b21 ]
  %75 = icmp slt i16 %73, %74
  br i1 %75, label %b19, label %b18

b18:
  br label %b28

b19:
  %76 = add nsw i16 %73, 1
  %77 = getelementptr inbounds i8, ptr @_queue, i16 %73
  %78 = load i8, ptr %77, !tbaa !7
  %79 = zext i8 %78 to i16
  br label %b20

b20:
  %80 = phi i16 [ %74, %b19 ], [ %113, %b27 ]
  %81 = phi i16 [ 0, %b19 ], [ %114, %b27 ]
  %82 = icmp slt i16 %81, %2
  br i1 %82, label %b22, label %b21

b21:
  br label %b17

b22:
  %83 = mul nsw i16 %79, 32
  %84 = add nsw i16 %83, %81
  %85 = getelementptr inbounds i8, ptr @_nxt, i16 %84
  %86 = load i8, ptr %85, !tbaa !7
  %87 = zext i8 %86 to i16
  %88 = getelementptr inbounds i8, ptr @_fail, i16 %79
  %89 = load i8, ptr %88, !tbaa !7
  %90 = zext i8 %89 to i16
  %91 = mul nsw i16 %90, 32
  %92 = add nsw i16 %91, %81
  %93 = getelementptr inbounds i8, ptr @_nxt, i16 %92
  %94 = load i8, ptr %93, !tbaa !7
  %95 = zext i8 %94 to i16
  %96 = icmp ne i16 %87, 0
  br i1 %96, label %b24, label %b23

b23:
  %97 = trunc i16 %95 to i8
  %98 = mul nsw i16 %79, 32
  %99 = add nsw i16 %98, %81
  %100 = getelementptr inbounds i8, ptr @_nxt, i16 %99
  store i8 %97, ptr %100, !tbaa !7
  br label %b27

b24:
  %101 = trunc i16 %95 to i8
  %102 = getelementptr inbounds i8, ptr @_fail, i16 %87
  store i8 %101, ptr %102, !tbaa !7
  %103 = getelementptr inbounds i8, ptr @_hit, i16 %87
  %104 = load i8, ptr %103, !tbaa !7
  %105 = zext i8 %104 to i16
  %106 = icmp eq i16 %105, 0
  br i1 %106, label %b26, label %b25

b25:
  %107 = trunc i16 %87 to i8
  %108 = add nsw i16 %80, 1
  %109 = getelementptr inbounds i8, ptr @_queue, i16 %80
  store i8 %107, ptr %109, !tbaa !7
  br label %b27

b26:
  %110 = getelementptr inbounds i8, ptr @_hit, i16 %95
  %111 = load i8, ptr %110, !tbaa !7
  %112 = getelementptr inbounds i8, ptr @_hit, i16 %87
  store i8 %111, ptr %112, !tbaa !7
  br label %b25

b27:
  %113 = phi i16 [ %108, %b25 ], [ %80, %b23 ]
  %114 = add nsw i16 %81, 1
  br label %b20

b28:
  %115 = phi i16 [ 0, %b18 ], [ %124, %b32 ]
  %116 = phi i16 [ 0, %b18 ], [ %126, %b32 ]
  %117 = phi i32 [ 0, %b18 ], [ %127, %b32 ]
  %118 = phi i32 [ 0, %b18 ], [ %128, %b32 ]
  %119 = phi i32 [ 0, %b18 ], [ %129, %b32 ]
  %120 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef %1, i16 noundef 1024)
  %121 = icmp sgt i16 %120, 0
  br i1 %121, label %b30, label %b29

b29:
  store i32 %118, ptr %0, !tbaa !11
  %122 = sext i16 %116 to i32
  %123 = add nsw i32 %117, %122
  ret i32 %123

b30:
  br label %b31

b31:
  %124 = phi i16 [ %115, %b30 ], [ %141, %b36 ]
  %125 = phi i16 [ 0, %b30 ], [ %160, %b36 ]
  %126 = phi i16 [ %116, %b30 ], [ %157, %b36 ]
  %127 = phi i32 [ %117, %b30 ], [ %158, %b36 ]
  %128 = phi i32 [ %118, %b30 ], [ %147, %b36 ]
  %129 = phi i32 [ %119, %b30 ], [ %159, %b36 ]
  %130 = icmp slt i16 %125, %120
  br i1 %130, label %b33, label %b32

b32:
  br label %b28

b33:
  %131 = getelementptr inbounds i8, ptr %1, i16 %125
  %132 = load i8, ptr %131, !tbaa !7
  %133 = mul nsw i16 %124, 32
  %134 = zext i8 %132 to i16
  %135 = getelementptr inbounds i8, ptr @_classof, i16 %134
  %136 = load i8, ptr %135, !tbaa !7
  %137 = zext i8 %136 to i16
  %138 = add nsw i16 %133, %137
  %139 = getelementptr inbounds i8, ptr @_nxt, i16 %138
  %140 = load i8, ptr %139, !tbaa !7
  %141 = zext i8 %140 to i16
  %142 = getelementptr inbounds i8, ptr @_hit, i16 %141
  %143 = load i8, ptr %142, !tbaa !7
  %144 = zext i8 %143 to i16
  %145 = icmp ne i16 %144, 0
  br i1 %145, label %b35, label %b34

b34:
  %146 = phi i16 [ 1, %b35 ], [ %126, %b33 ]
  %147 = phi i32 [ %156, %b35 ], [ %128, %b33 ]
  %148 = zext i8 %132 to i16
  %149 = icmp eq i16 %148, 10
  br i1 %149, label %b37, label %b36

b35:
  %150 = shl i32 %128, 1
  %151 = add nsw i32 %150, %129
  %152 = getelementptr inbounds i8, ptr @_hit, i16 %141
  %153 = load i8, ptr %152, !tbaa !7
  %154 = zext i8 %153 to i32
  %155 = add nsw i32 %151, %154
  %156 = and i32 %155, 536870911
  br label %b34

b36:
  %157 = phi i16 [ 0, %b37 ], [ %146, %b34 ]
  %158 = phi i32 [ %162, %b37 ], [ %127, %b34 ]
  %159 = add nsw i32 %129, 1
  %160 = add nsw i16 %125, 1
  br label %b31

b37:
  %161 = sext i16 %146 to i32
  %162 = add nsw i32 %127, %161
  br label %b36
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
