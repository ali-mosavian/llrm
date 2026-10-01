target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16-n8:16:32"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [13 x i8] c"\08\00\06\00\06\00Rovers\00"
@$str3 = internal constant [13 x i8] c"\08\00\06\00\06\00United\00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00Athletic\00"
@$str5 = internal constant [16 x i8] c"\08\00\09\00\09\00 lead on \00"
@$str6 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str7 = internal constant [13 x i8] c"\08\00\06\00\06\00 from \00"

define internal void @Team.record(ptr addrspace(1) nonnull dereferenceable(8) noalias nocapture %0, i16 %1, i16 %2) addrspace(1) memory(argmem: readwrite) willreturn {
b1:
  %3 = icmp sgt i16 %1, %2
  br i1 %3, label %b2, label %b3

b2:
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %5 = load i16, ptr addrspace(1) %4
  %6 = add i16 %5, 1
  store i16 %6, ptr addrspace(1) %4
  br label %b4

b3:
  %7 = icmp eq i16 %1, %2
  br i1 %7, label %b5, label %b6

b4:
  ret void

b5:
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %9 = load i16, ptr addrspace(1) %8
  %10 = add i16 %9, 1
  store i16 %10, ptr addrspace(1) %8
  br label %b4

b6:
  %11 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %12 = load i16, ptr addrspace(1) %11
  %13 = add i16 %12, 1
  store i16 %13, ptr addrspace(1) %11
  br label %b4
}

define i16 @main() addrspace(1) memory(readwrite, argmem: none) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [8 x i8]
  %2 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  %3 = getelementptr i8, ptr @$str1, i16 6
  %4 = call addrspace(1) ptr @N$BGRW(ptr %3, i16 3, i16 2)
  %5 = getelementptr i8, ptr %4, i16 0
  %6 = getelementptr i8, ptr @$str2, i16 6
  store ptr %6, ptr %5
  %7 = getelementptr i8, ptr %4, i16 2
  %8 = getelementptr i8, ptr @$str3, i16 6
  store ptr %8, ptr %7
  %9 = getelementptr i8, ptr %4, i16 4
  %10 = getelementptr i8, ptr @$str4, i16 6
  store ptr %10, ptr %9
  %11 = getelementptr i8, ptr %4, i16 -4
  %12 = load i16, ptr %11
  %13 = sub i16 0, %12
  %14 = icmp eq i16 %12, 0
  br i1 %14, label %b5, label %138

b3:
  %15 = phi ptr [ %20, %b3 ], [ %3, %138 ]
  %16 = phi i16 [ %28, %b3 ], [ %13, %138 ]
  %lsr.iv4 = phi i16 [ %lsr.iv.next, %b3 ], [ 0, %138 ]
  %17 = getelementptr i8, ptr %4, i16 %lsr.iv4
  %18 = getelementptr i8, ptr %15, i16 -4
  %19 = load i16, ptr %18
  %20 = call addrspace(1) ptr @N$BGRW(ptr %15, i16 1, i16 8)
  %21 = mul i16 %19, 8
  %22 = getelementptr i8, ptr %20, i16 %21
  %23 = load ptr, ptr %17
  %24 = call addrspace(1) ptr @N$BCLN(ptr %23, i16 1)
  store ptr %24, ptr %22
  %25 = getelementptr i8, ptr %22, i16 2
  store i16 0, ptr %25
  %26 = getelementptr i8, ptr %22, i16 4
  store i16 0, ptr %26
  %27 = getelementptr i8, ptr %22, i16 6
  store i16 0, ptr %27
  %28 = add i16 %16, 1
  %lsr.iv.next = add i16 %lsr.iv4, 2
  %29 = icmp ne i16 %28, 0
  br i1 %29, label %b3, label %139

b5:
  %30 = phi ptr [ %3, %b1 ], [ %140, %139 ]
  %31 = icmp ne ptr %4, null
  br i1 %31, label %b7, label %b6

b6:
  call addrspace(1) void @N$BDRP(ptr %4)
  %32 = getelementptr i8, ptr %30, i16 -4
  %33 = load i16, ptr %32
  %34 = icmp ne i16 %33, 0
  br i1 %34, label %b11, label %b12

b7:
  %35 = load i16, ptr %11
  %36 = sub i16 0, %35
  %37 = icmp eq i16 %35, 0
  br i1 %37, label %b6, label %141

b10:
  %38 = phi i16 [ %41, %b10 ], [ %36, %141 ]
  %lsr.iv11 = phi i16 [ %lsr.iv.next1, %b10 ], [ 0, %141 ]
  %39 = getelementptr i8, ptr %4, i16 %lsr.iv11
  %40 = load ptr, ptr %39
  call addrspace(1) void @N$BDRP(ptr %40)
  %41 = add i16 %38, 1
  %lsr.iv.next1 = add i16 %lsr.iv11, 2
  %42 = icmp ne i16 %41, 0
  br i1 %42, label %b10, label %142

b11:
  %43 = getelementptr i8, ptr %30, i16 0
  %44 = addrspacecast ptr %43 to ptr addrspace(1)
  call addrspace(1) void @Team.record(ptr addrspace(1) %44, i16 2, i16 1)
  %45 = load i16, ptr %32
  %46 = icmp ne i16 %45, 0
  br i1 %46, label %b13, label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  call addrspace(1) void @Team.record(ptr addrspace(1) %44, i16 0, i16 0)
  %47 = load i16, ptr %32
  %48 = icmp ugt i16 %47, 1
  br i1 %48, label %b15, label %b16

b14:
  call addrspace(1) void @N$EBND()
  unreachable

b15:
  %49 = getelementptr i8, ptr %30, i16 8
  %50 = addrspacecast ptr %49 to ptr addrspace(1)
  call addrspace(1) void @Team.record(ptr addrspace(1) %50, i16 3, i16 0)
  %51 = load i16, ptr %32
  %52 = icmp ugt i16 %51, 1
  br i1 %52, label %b17, label %b18

b16:
  call addrspace(1) void @N$EBND()
  unreachable

b17:
  call addrspace(1) void @Team.record(ptr addrspace(1) %50, i16 1, i16 2)
  %53 = load i16, ptr %32
  %54 = icmp ugt i16 %53, 2
  br i1 %54, label %b19, label %b20

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %55 = getelementptr i8, ptr %30, i16 16
  %56 = addrspacecast ptr %55 to ptr addrspace(1)
  call addrspace(1) void @Team.record(ptr addrspace(1) %56, i16 1, i16 1)
  %57 = load i16, ptr %32
  %58 = icmp ugt i16 %57, 2
  br i1 %58, label %b21, label %b22

b20:
  call addrspace(1) void @N$EBND()
  unreachable

b21:
  call addrspace(1) void @Team.record(ptr addrspace(1) %56, i16 0, i16 4)
  %59 = load i16, ptr %32
  %60 = sub i16 0, %59
  %61 = icmp eq i16 %59, 0
  br i1 %61, label %b26, label %143

b22:
  call addrspace(1) void @N$EBND()
  unreachable

b24:
  %62 = phi ptr [ %67, %b24 ], [ %3, %143 ]
  %63 = phi i16 [ %77, %b24 ], [ %60, %143 ]
  %lsr.iv21 = phi i16 [ %lsr.iv.next2, %b24 ], [ 0, %143 ]
  %64 = getelementptr i8, ptr %30, i16 %lsr.iv21
  %65 = getelementptr i8, ptr %62, i16 -4
  %66 = load i16, ptr %65
  %67 = call addrspace(1) ptr @N$BGRW(ptr %62, i16 1, i16 2)
  %68 = mul i16 %66, 2
  %69 = getelementptr i8, ptr %67, i16 %68
  %70 = addrspacecast ptr %64 to ptr addrspace(1)
  %71 = getelementptr i8, ptr addrspace(1) %70, i16 2
  %72 = load i16, ptr addrspace(1) %71
  %73 = mul i16 %72, 3
  %74 = getelementptr i8, ptr addrspace(1) %70, i16 4
  %75 = load i16, ptr addrspace(1) %74
  %76 = add i16 %73, %75
  store i16 %76, ptr %69
  %77 = add i16 %63, 1
  %lsr.iv.next2 = add i16 %lsr.iv21, 8
  %78 = icmp ne i16 %77, 0
  br i1 %78, label %b24, label %144

b26:
  %79 = phi ptr [ %3, %b21 ], [ %145, %144 ]
  %80 = getelementptr i8, ptr %79, i16 -4
  %81 = load i16, ptr %80
  %82 = addrspacecast ptr %79 to ptr addrspace(1)
  store i16 %81, ptr %1, !tbaa !2
  %83 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %81, ptr %83, !tbaa !2
  %84 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %82, ptr %84, !tbaa !2
  %85 = addrspacecast ptr %1 to ptr addrspace(1)
  %86 = call addrspace(1) i32 @"best[i16]"(ptr addrspace(1) %85)
  %87 = addrspacecast ptr %2 to ptr addrspace(1)
  store i32 %86, ptr addrspace(1) %87, !tbaa !2
  %88 = load i16, ptr %2, !tbaa !2
  %89 = getelementptr inbounds i8, ptr %2, i16 2
  %90 = load i16, ptr %89, !tbaa !2
  %91 = load i16, ptr %32
  %92 = icmp ult i16 %88, %91
  br i1 %92, label %b27, label %b28

b27:
  %93 = mul i16 %88, 8
  %94 = getelementptr i8, ptr %30, i16 %93
  %95 = load ptr, ptr %94
  call addrspace(1) void @N$PS(ptr %95)
  %96 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %96)
  call addrspace(1) void @N$PI2(i16 %90)
  call addrspace(1) void @N$PN()
  %97 = load i16, ptr %80
  %98 = getelementptr inbounds i8, ptr %0, i16 2
  %99 = getelementptr inbounds i8, ptr %0, i16 4
  %100 = addrspacecast ptr %0 to ptr addrspace(1)
  %101 = getelementptr i8, ptr @$str6, i16 6
  %102 = getelementptr i8, ptr @$str7, i16 6
  br label %b30

b28:
  call addrspace(1) void @N$EBND()
  unreachable

b30:
  %103 = phi i16 [ 0, %b27 ], [ %112, %b36 ]
  %104 = icmp ult i16 %103, %97
  br i1 %104, label %b31, label %b33

b31:
  %105 = mul i16 %103, 2
  %106 = getelementptr i8, ptr addrspace(1) %82, i16 %105
  %107 = load i16, ptr addrspace(1) %106
  %108 = icmp sge i16 %107, 2
  br i1 %108, label %b34, label %b36

b33:
  call addrspace(1) void @N$BDRP(ptr %79)
  call addrspace(1) void @N$BDRP(ptr null)
  %109 = icmp ne ptr %30, null
  br i1 %109, label %b41, label %b40

b34:
  %110 = load i16, ptr %32
  %111 = icmp ult i16 %103, %110
  br i1 %111, label %b38, label %b39

b36:
  %112 = add i16 %103, 1
  br label %b30

b38:
  %113 = mul i16 %103, 8
  %114 = getelementptr i8, ptr %30, i16 %113
  %115 = load ptr, ptr %114
  %116 = getelementptr i8, ptr %114, i16 2
  %117 = load i16, ptr %116
  %118 = getelementptr i8, ptr %114, i16 4
  %119 = load i16, ptr %118
  %120 = getelementptr i8, ptr %114, i16 6
  %121 = load i16, ptr %120
  %122 = call addrspace(1) ptr @N$BCLN(ptr %115, i16 1)
  %123 = getelementptr i8, ptr %122, i16 -4
  %124 = load i16, ptr %123
  %125 = addrspacecast ptr %122 to ptr addrspace(1)
  store i16 %124, ptr %0, !tbaa !2
  store i16 %124, ptr %98, !tbaa !2
  store ptr addrspace(1) %125, ptr %99, !tbaa !2
  %126 = mul i16 %117, 3
  %127 = add i16 %126, %119
  %128 = add i16 %117, %119
  %129 = add i16 %128, %121
  call addrspace(1) void @N$PV(ptr addrspace(1) %100)
  call addrspace(1) void @N$PS(ptr %101)
  call addrspace(1) void @N$PI2(i16 %127)
  call addrspace(1) void @N$PS(ptr %102)
  call addrspace(1) void @N$PI2(i16 %129)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %122)
  br label %b36

b39:
  call addrspace(1) void @N$EBND()
  unreachable

b40:
  call addrspace(1) void @N$BDRP(ptr %30)
  ret i16 0

b41:
  %130 = load i16, ptr %32
  %131 = sub i16 0, %130
  %132 = icmp eq i16 %130, 0
  br i1 %132, label %b40, label %146

b44:
  %133 = phi i16 [ %136, %b44 ], [ %131, %146 ]
  %lsr.iv31 = phi i16 [ %lsr.iv.next3, %b44 ], [ 0, %146 ]
  %134 = getelementptr i8, ptr %30, i16 %lsr.iv31
  %135 = load ptr, ptr %134
  call addrspace(1) void @N$BDRP(ptr %135)
  %136 = add i16 %133, 1
  %lsr.iv.next3 = add i16 %lsr.iv31, 8
  %137 = icmp ne i16 %136, 0
  br i1 %137, label %b44, label %147

138:
  br label %b3

139:
  %140 = phi ptr [ %20, %b3 ]
  br label %b5

141:
  br label %b10

142:
  br label %b6

143:
  br label %b24

144:
  %145 = phi ptr [ %67, %b24 ]
  br label %b26

146:
  br label %b44

147:
  br label %b40
}

define internal i32 @"best[i16]"(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) memory(readwrite, argmem: read) {
b1:
  %1 = alloca [4 x i8]
  %2 = load i16, ptr addrspace(1) %0
  %3 = icmp ne i16 %2, 0
  br i1 %3, label %b2, label %b3

b2:
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %5 = load ptr addrspace(1), ptr addrspace(1) %4
  %6 = getelementptr i8, ptr addrspace(1) %5, i16 0
  %7 = load i16, ptr addrspace(1) %6
  br label %b4

b3:
  call addrspace(1) void @N$EBND()
  unreachable

b4:
  %8 = phi i16 [ 0, %b2 ], [ %23, %b10 ]
  %9 = phi i16 [ %7, %b2 ], [ %21, %b10 ]
  %10 = phi i16 [ 0, %b2 ], [ %22, %b10 ]
  %11 = icmp ult i16 %8, %2
  br i1 %11, label %b5, label %b7

b5:
  %12 = mul i16 %8, 2
  %13 = getelementptr i8, ptr addrspace(1) %5, i16 %12
  %14 = load i16, ptr addrspace(1) %13
  %15 = icmp sgt i16 %14, %9
  br i1 %15, label %b8, label %24

b7:
  %16 = phi i16 [ %10, %b4 ]
  %17 = phi i16 [ %9, %b4 ]
  store i16 %16, ptr %1, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %17, ptr %18, !tbaa !2
  %19 = addrspacecast ptr %1 to ptr addrspace(1)
  %20 = load i32, ptr addrspace(1) %19, !tbaa !2
  ret i32 %20

b8:
  br label %b10

b10:
  %21 = phi i16 [ %14, %b8 ], [ %9, %24 ]
  %22 = phi i16 [ %8, %b8 ], [ %10, %24 ]
  %23 = add i16 %8, 1
  br label %b4

24:
  br label %b10
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare ptr @N$BCLN(ptr, i16) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare void @N$EBND() addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PV(ptr addrspace(1)) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
