target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@_patterns = internal constant [81 x i8] c"Oliver|Scrooge|Copperfield|Dombey|Marley|Micawber|Heep|ghost|gentleman|Tiny Tim|\00"
@_queue = internal global [80 x i8] zeroinitializer
@_nxt = internal global [2560 x i8] zeroinitializer
@_fail = internal global [80 x i8] zeroinitializer
@_classof = internal global [256 x i8] zeroinitializer
@_hit = internal global [80 x i8] zeroinitializer

define cc 1005 i32 @"_bench_grep@3"(ptr noundef %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca ptr
  %2 = alloca i32
  %3 = alloca [1024 x i8]
  %4 = alloca ptr
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i32
  %16 = alloca i32
  %17 = alloca i32
  %18 = alloca i8
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i8
  store ptr %0, ptr %1, !tbaa !21
  store i16 1, ptr %5, !tbaa !9
  store i16 1, ptr %6, !tbaa !9
  store i16 1, ptr %7, !tbaa !9
  store i16 0, ptr %8, !tbaa !9
  store i16 0, ptr %9, !tbaa !9
  store i16 0, ptr %10, !tbaa !9
  store i16 0, ptr %14, !tbaa !9
  store i32 0, ptr %15, !tbaa !11
  store i32 0, ptr %16, !tbaa !11
  store i32 0, ptr %17, !tbaa !11
  store ptr @_patterns, ptr %4, !tbaa !21
  br label %b2

b2:
  %22 = load ptr, ptr %4, !tbaa !21
  %23 = load i8, ptr %22, !tbaa !7
  %24 = sext i8 %23 to i16
  %25 = icmp ne i16 %24, 0
  br i1 %25, label %b4, label %b3

b3:
  store i16 1, ptr %11, !tbaa !9
  br label %b12

b4:
  call void @llvm.lifetime.start.p0(i64 1, ptr %18)
  %26 = load ptr, ptr %4, !tbaa !21
  %27 = load i8, ptr %26, !tbaa !7
  store i8 %27, ptr %18, !tbaa !7
  %28 = load i8, ptr %18, !tbaa !7
  %29 = zext i8 %28 to i16
  %30 = icmp eq i16 %29, 124
  br i1 %30, label %b6, label %b5

b5:
  %31 = load i8, ptr %18, !tbaa !7
  %32 = zext i8 %31 to i16
  %33 = getelementptr inbounds i8, ptr @_classof, i16 %32
  %34 = load i8, ptr %33, !tbaa !7
  %35 = zext i8 %34 to i16
  %36 = icmp eq i16 %35, 0
  br i1 %36, label %b9, label %b8

b6:
  %37 = load i16, ptr %7, !tbaa !9
  %38 = add nsw i16 %37, 1
  store i16 %38, ptr %7, !tbaa !9
  %39 = trunc i16 %37 to i8
  %40 = load i16, ptr %8, !tbaa !9
  %41 = getelementptr inbounds i8, ptr @_hit, i16 %40
  store i8 %39, ptr %41, !tbaa !7
  store i16 0, ptr %8, !tbaa !9
  br label %b7

b7:
  %42 = load ptr, ptr %4, !tbaa !21
  %43 = getelementptr inbounds i8, ptr %42, i16 1
  store ptr %43, ptr %4, !tbaa !21
  br label %b2

b8:
  %44 = load i16, ptr %8, !tbaa !9
  %45 = mul nsw i16 %44, 32
  %46 = load i8, ptr %18, !tbaa !7
  %47 = zext i8 %46 to i16
  %48 = getelementptr inbounds i8, ptr @_classof, i16 %47
  %49 = load i8, ptr %48, !tbaa !7
  %50 = zext i8 %49 to i16
  %51 = add nsw i16 %45, %50
  %52 = getelementptr inbounds i8, ptr @_nxt, i16 %51
  %53 = load i8, ptr %52, !tbaa !7
  %54 = zext i8 %53 to i16
  %55 = icmp eq i16 %54, 0
  br i1 %55, label %b11, label %b10

b9:
  %56 = load i16, ptr %5, !tbaa !9
  %57 = add nsw i16 %56, 1
  store i16 %57, ptr %5, !tbaa !9
  %58 = trunc i16 %56 to i8
  %59 = load i8, ptr %18, !tbaa !7
  %60 = zext i8 %59 to i16
  %61 = getelementptr inbounds i8, ptr @_classof, i16 %60
  store i8 %58, ptr %61, !tbaa !7
  br label %b8

b10:
  %62 = load i16, ptr %8, !tbaa !9
  %63 = mul nsw i16 %62, 32
  %64 = load i8, ptr %18, !tbaa !7
  %65 = zext i8 %64 to i16
  %66 = getelementptr inbounds i8, ptr @_classof, i16 %65
  %67 = load i8, ptr %66, !tbaa !7
  %68 = zext i8 %67 to i16
  %69 = add nsw i16 %63, %68
  %70 = getelementptr inbounds i8, ptr @_nxt, i16 %69
  %71 = load i8, ptr %70, !tbaa !7
  %72 = zext i8 %71 to i16
  store i16 %72, ptr %8, !tbaa !9
  call void @llvm.lifetime.end.p0(i64 1, ptr %18)
  br label %b7

b11:
  %73 = load i16, ptr %6, !tbaa !9
  %74 = add nsw i16 %73, 1
  store i16 %74, ptr %6, !tbaa !9
  %75 = trunc i16 %73 to i8
  %76 = load i16, ptr %8, !tbaa !9
  %77 = mul nsw i16 %76, 32
  %78 = load i8, ptr %18, !tbaa !7
  %79 = zext i8 %78 to i16
  %80 = getelementptr inbounds i8, ptr @_classof, i16 %79
  %81 = load i8, ptr %80, !tbaa !7
  %82 = zext i8 %81 to i16
  %83 = add nsw i16 %77, %82
  %84 = getelementptr inbounds i8, ptr @_nxt, i16 %83
  store i8 %75, ptr %84, !tbaa !7
  br label %b10

b12:
  %85 = load i16, ptr %11, !tbaa !9
  %86 = load i16, ptr %5, !tbaa !9
  %87 = icmp slt i16 %85, %86
  br i1 %87, label %b14, label %b13

b13:
  br label %b17

b14:
  %88 = load i16, ptr %11, !tbaa !9
  %89 = getelementptr inbounds i8, ptr @_nxt, i16 %88
  %90 = load i8, ptr %89, !tbaa !7
  %91 = zext i8 %90 to i16
  %92 = icmp ne i16 %91, 0
  br i1 %92, label %b16, label %b15

b15:
  %93 = load i16, ptr %11, !tbaa !9
  %94 = add nsw i16 %93, 1
  store i16 %94, ptr %11, !tbaa !9
  br label %b12

b16:
  %95 = load i16, ptr %11, !tbaa !9
  %96 = getelementptr inbounds i8, ptr @_nxt, i16 %95
  %97 = load i8, ptr %96, !tbaa !7
  %98 = load i16, ptr %10, !tbaa !9
  %99 = add nsw i16 %98, 1
  store i16 %99, ptr %10, !tbaa !9
  %100 = getelementptr inbounds i8, ptr @_queue, i16 %98
  store i8 %97, ptr %100, !tbaa !7
  br label %b15

b17:
  %101 = load i16, ptr %9, !tbaa !9
  %102 = load i16, ptr %10, !tbaa !9
  %103 = icmp slt i16 %101, %102
  br i1 %103, label %b19, label %b18

b18:
  store i16 0, ptr %8, !tbaa !9
  br label %b28

b19:
  %104 = load i16, ptr %9, !tbaa !9
  %105 = add nsw i16 %104, 1
  store i16 %105, ptr %9, !tbaa !9
  %106 = getelementptr inbounds i8, ptr @_queue, i16 %104
  %107 = load i8, ptr %106, !tbaa !7
  %108 = zext i8 %107 to i16
  store i16 %108, ptr %8, !tbaa !9
  store i16 0, ptr %11, !tbaa !9
  br label %b20

b20:
  %109 = load i16, ptr %11, !tbaa !9
  %110 = load i16, ptr %5, !tbaa !9
  %111 = icmp slt i16 %109, %110
  br i1 %111, label %b22, label %b21

b21:
  br label %b17

b22:
  call void @llvm.lifetime.start.p0(i64 2, ptr %19)
  call void @llvm.lifetime.start.p0(i64 2, ptr %20)
  %112 = load i16, ptr %8, !tbaa !9
  %113 = mul nsw i16 %112, 32
  %114 = load i16, ptr %11, !tbaa !9
  %115 = add nsw i16 %113, %114
  %116 = getelementptr inbounds i8, ptr @_nxt, i16 %115
  %117 = load i8, ptr %116, !tbaa !7
  %118 = zext i8 %117 to i16
  store i16 %118, ptr %19, !tbaa !9
  %119 = load i16, ptr %8, !tbaa !9
  %120 = getelementptr inbounds i8, ptr @_fail, i16 %119
  %121 = load i8, ptr %120, !tbaa !7
  %122 = zext i8 %121 to i16
  %123 = mul nsw i16 %122, 32
  %124 = load i16, ptr %11, !tbaa !9
  %125 = add nsw i16 %123, %124
  %126 = getelementptr inbounds i8, ptr @_nxt, i16 %125
  %127 = load i8, ptr %126, !tbaa !7
  %128 = zext i8 %127 to i16
  store i16 %128, ptr %20, !tbaa !9
  %129 = load i16, ptr %19, !tbaa !9
  %130 = icmp ne i16 %129, 0
  br i1 %130, label %b24, label %b23

b23:
  %131 = load i16, ptr %20, !tbaa !9
  %132 = trunc i16 %131 to i8
  %133 = load i16, ptr %8, !tbaa !9
  %134 = mul nsw i16 %133, 32
  %135 = load i16, ptr %11, !tbaa !9
  %136 = add nsw i16 %134, %135
  %137 = getelementptr inbounds i8, ptr @_nxt, i16 %136
  store i8 %132, ptr %137, !tbaa !7
  br label %b27

b24:
  %138 = load i16, ptr %20, !tbaa !9
  %139 = trunc i16 %138 to i8
  %140 = load i16, ptr %19, !tbaa !9
  %141 = getelementptr inbounds i8, ptr @_fail, i16 %140
  store i8 %139, ptr %141, !tbaa !7
  %142 = load i16, ptr %19, !tbaa !9
  %143 = getelementptr inbounds i8, ptr @_hit, i16 %142
  %144 = load i8, ptr %143, !tbaa !7
  %145 = zext i8 %144 to i16
  %146 = icmp eq i16 %145, 0
  br i1 %146, label %b26, label %b25

b25:
  %147 = load i16, ptr %19, !tbaa !9
  %148 = trunc i16 %147 to i8
  %149 = load i16, ptr %10, !tbaa !9
  %150 = add nsw i16 %149, 1
  store i16 %150, ptr %10, !tbaa !9
  %151 = getelementptr inbounds i8, ptr @_queue, i16 %149
  store i8 %148, ptr %151, !tbaa !7
  br label %b27

b26:
  %152 = load i16, ptr %20, !tbaa !9
  %153 = getelementptr inbounds i8, ptr @_hit, i16 %152
  %154 = load i8, ptr %153, !tbaa !7
  %155 = load i16, ptr %19, !tbaa !9
  %156 = getelementptr inbounds i8, ptr @_hit, i16 %155
  store i8 %154, ptr %156, !tbaa !7
  br label %b25

b27:
  call void @llvm.lifetime.end.p0(i64 2, ptr %20)
  call void @llvm.lifetime.end.p0(i64 2, ptr %19)
  %157 = load i16, ptr %11, !tbaa !9
  %158 = add nsw i16 %157, 1
  store i16 %158, ptr %11, !tbaa !9
  br label %b20

b28:
  %159 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef %3, i16 noundef 1024)
  store i16 %159, ptr %12, !tbaa !9
  %160 = icmp sgt i16 %159, 0
  br i1 %160, label %b30, label %b29

b29:
  %161 = load i32, ptr %16, !tbaa !11
  %162 = load ptr, ptr %1, !tbaa !21
  store i32 %161, ptr %162, !tbaa !11
  %163 = load i32, ptr %15, !tbaa !11
  %164 = load i16, ptr %14, !tbaa !9
  %165 = sext i16 %164 to i32
  %166 = add nsw i32 %163, %165
  store i32 %166, ptr %2, !tbaa !11
  %167 = load i32, ptr %2, !tbaa !11
  ret i32 %167

b30:
  store i16 0, ptr %13, !tbaa !9
  br label %b31

b31:
  %168 = load i16, ptr %13, !tbaa !9
  %169 = load i16, ptr %12, !tbaa !9
  %170 = icmp slt i16 %168, %169
  br i1 %170, label %b33, label %b32

b32:
  br label %b28

b33:
  call void @llvm.lifetime.start.p0(i64 1, ptr %21)
  %171 = load i16, ptr %13, !tbaa !9
  %172 = getelementptr inbounds i8, ptr %3, i16 %171
  %173 = load i8, ptr %172, !tbaa !7
  store i8 %173, ptr %21, !tbaa !7
  %174 = load i16, ptr %8, !tbaa !9
  %175 = mul nsw i16 %174, 32
  %176 = load i8, ptr %21, !tbaa !7
  %177 = zext i8 %176 to i16
  %178 = getelementptr inbounds i8, ptr @_classof, i16 %177
  %179 = load i8, ptr %178, !tbaa !7
  %180 = zext i8 %179 to i16
  %181 = add nsw i16 %175, %180
  %182 = getelementptr inbounds i8, ptr @_nxt, i16 %181
  %183 = load i8, ptr %182, !tbaa !7
  %184 = zext i8 %183 to i16
  store i16 %184, ptr %8, !tbaa !9
  %185 = load i16, ptr %8, !tbaa !9
  %186 = getelementptr inbounds i8, ptr @_hit, i16 %185
  %187 = load i8, ptr %186, !tbaa !7
  %188 = zext i8 %187 to i16
  %189 = icmp ne i16 %188, 0
  br i1 %189, label %b35, label %b34

b34:
  %190 = load i8, ptr %21, !tbaa !7
  %191 = zext i8 %190 to i16
  %192 = icmp eq i16 %191, 10
  br i1 %192, label %b37, label %b36

b35:
  store i16 1, ptr %14, !tbaa !9
  %193 = load i32, ptr %16, !tbaa !11
  %194 = shl i32 %193, 1
  %195 = load i32, ptr %17, !tbaa !11
  %196 = add nsw i32 %194, %195
  %197 = load i16, ptr %8, !tbaa !9
  %198 = getelementptr inbounds i8, ptr @_hit, i16 %197
  %199 = load i8, ptr %198, !tbaa !7
  %200 = zext i8 %199 to i32
  %201 = add nsw i32 %196, %200
  %202 = and i32 %201, 536870911
  store i32 %202, ptr %16, !tbaa !11
  br label %b34

b36:
  %203 = load i32, ptr %17, !tbaa !11
  %204 = add nsw i32 %203, 1
  store i32 %204, ptr %17, !tbaa !11
  call void @llvm.lifetime.end.p0(i64 1, ptr %21)
  %205 = load i16, ptr %13, !tbaa !9
  %206 = add nsw i16 %205, 1
  store i16 %206, ptr %13, !tbaa !9
  br label %b31

b37:
  %207 = load i32, ptr %15, !tbaa !11
  %208 = load i16, ptr %14, !tbaa !9
  %209 = sext i16 %208 to i32
  %210 = add nsw i32 %207, %209
  store i32 %210, ptr %15, !tbaa !11
  store i16 0, ptr %14, !tbaa !9
  br label %b36
}

define i16 @_main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i32
  %2 = alloca i32
  %3 = call cc 1005 addrspace(1) i32 @"_bench_grep@3"(ptr noundef %1)
  store i32 %3, ptr %2, !tbaa !11
  %4 = load i32, ptr %2, !tbaa !11
  %5 = call cc 1005 addrspace(1) i16 @"_report@3"(i32 noundef %4)
  %6 = load i32, ptr %1, !tbaa !11
  %7 = call cc 1005 addrspace(1) i16 @"_report@3"(i32 noundef %6)
  store i16 0, ptr %0, !tbaa !9
  %8 = load i16, ptr %0, !tbaa !9
  ret i16 %8
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
