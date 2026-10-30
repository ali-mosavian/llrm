target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal cc 1005 i16 @trace(ptr addrspace(1) noalias readonly dereferenceable(10) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %3, !tbaa !2
  %4 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %2, !tbaa !2
  store i16 %4, ptr %1, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %2, !tbaa !2
  %6 = load i16, ptr %1, !tbaa !2
  %7 = icmp ult i16 %5, %6
  br i1 %7, label %b3, label %b5

b3:
  %8 = load i16, ptr %3, !tbaa !2
  %9 = load i16, ptr %2, !tbaa !2
  %10 = load i16, ptr %2, !tbaa !2
  %11 = load i16, ptr addrspace(1) %0, !range !5
  %12 = icmp ult i16 %9, %11
  br i1 %12, label %b6, label %b7

b4:
  %13 = load i16, ptr %2, !tbaa !2
  %14 = add nuw i16 %13, 1
  store i16 %14, ptr %2, !tbaa !2
  br label %b2

b5:
  %15 = load i16, ptr %3, !tbaa !2
  ret i16 %15

b6:
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %17 = load i16, ptr addrspace(1) %16, !range !6
  %18 = icmp ult i16 %10, %17
  br i1 %18, label %b8, label %b9

b7:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b8:
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %20 = load i16, ptr addrspace(1) %19
  %21 = mul i16 %20, 1
  %22 = mul i16 %9, %21
  %23 = mul i16 %10, 1
  %24 = add i16 %22, %23
  %25 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %26 = load ptr addrspace(1), ptr addrspace(1) %25
  %27 = mul i16 %24, 2
  %28 = getelementptr inbounds i8, ptr addrspace(1) %26, i16 %27
  %29 = load i16, ptr addrspace(1) %28
  %30 = add i16 %8, %29
  store i16 %30, ptr %3, !tbaa !2
  br label %b4

b9:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 void @bump(ptr addrspace(1) noalias readonly dereferenceable(10) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = load i16, ptr addrspace(1) %0, !range !7
  %2 = icmp ult i16 1, %1
  br i1 %2, label %b2, label %b3

b2:
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %4 = load i16, ptr addrspace(1) %3, !range !8
  %5 = icmp ult i16 2, %4
  br i1 %5, label %b4, label %b5

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b4:
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %7 = load i16, ptr addrspace(1) %6
  %8 = mul i16 %7, 1
  %9 = mul i16 1, %8
  %10 = add i16 %9, 2
  %11 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %12 = load ptr addrspace(1), ptr addrspace(1) %11
  %13 = mul i16 %10, 2
  %14 = getelementptr inbounds i8, ptr addrspace(1) %12, i16 %13
  %15 = load i16, ptr addrspace(1) %14
  %16 = add i16 %15, 100
  store i16 %16, ptr addrspace(1) %14
  ret void

b5:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca [10 x i8]
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [10 x i8]
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca [64 x i8]
  %24 = alloca i32
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca i16
  %29 = alloca i16
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca i16
  %34 = alloca i16
  %35 = alloca [24 x i8]
  %36 = alloca i8
  %37 = alloca i16
  %38 = alloca i16
  %39 = alloca i16
  %40 = alloca [18 x i8]
  store i16 3, ptr %37, !tbaa !2
  store i16 3, ptr %38, !tbaa !2
  store i16 9, ptr %39, !tbaa !2
  %41 = getelementptr inbounds i16, ptr %40, i16 0
  store i16 1, ptr %41, !tbaa !2
  %42 = getelementptr inbounds i16, ptr %40, i16 1
  store i16 2, ptr %42, !tbaa !2
  %43 = getelementptr inbounds i16, ptr %40, i16 2
  store i16 3, ptr %43, !tbaa !2
  %44 = getelementptr inbounds i16, ptr %40, i16 3
  store i16 4, ptr %44, !tbaa !2
  %45 = getelementptr inbounds i16, ptr %40, i16 4
  store i16 5, ptr %45, !tbaa !2
  %46 = getelementptr inbounds i16, ptr %40, i16 5
  store i16 6, ptr %46, !tbaa !2
  %47 = getelementptr inbounds i16, ptr %40, i16 6
  store i16 7, ptr %47, !tbaa !2
  %48 = getelementptr inbounds i16, ptr %40, i16 7
  store i16 8, ptr %48, !tbaa !2
  %49 = getelementptr inbounds i16, ptr %40, i16 8
  store i16 9, ptr %49, !tbaa !2
  store i8 7, ptr %36, !tbaa !2
  store i16 2, ptr %31, !tbaa !2
  store i16 3, ptr %32, !tbaa !2
  store i16 4, ptr %33, !tbaa !2
  store i16 24, ptr %34, !tbaa !2
  store i16 0, ptr %30, !tbaa !2
  store i16 2, ptr %29, !tbaa !2
  br label %b2

b2:
  %50 = load i16, ptr %30, !tbaa !2
  %51 = load i16, ptr %29, !tbaa !2
  %52 = icmp slt i16 %50, %51
  br i1 %52, label %b3, label %b5

b3:
  store i16 0, ptr %28, !tbaa !2
  store i16 3, ptr %27, !tbaa !2
  br label %b6

b4:
  %53 = load i16, ptr %30, !tbaa !2
  %54 = add nsw i16 %53, 1
  store i16 %54, ptr %30, !tbaa !2
  br label %b2

b5:
  %55 = getelementptr inbounds i8, ptr %35, i16 23
  store i8 1, ptr %55, !tbaa !2
  store i32 0, ptr %24, !tbaa !2
  store i16 2, ptr %18, !tbaa !2
  store i16 2, ptr %19, !tbaa !2
  store i16 2, ptr %20, !tbaa !2
  store i16 2, ptr %21, !tbaa !2
  store i16 16, ptr %22, !tbaa !2
  store i16 0, ptr %17, !tbaa !2
  store i16 2, ptr %16, !tbaa !2
  br label %b14

b6:
  %56 = load i16, ptr %28, !tbaa !2
  %57 = load i16, ptr %27, !tbaa !2
  %58 = icmp slt i16 %56, %57
  br i1 %58, label %b7, label %b9

b7:
  store i16 0, ptr %26, !tbaa !2
  store i16 4, ptr %25, !tbaa !2
  br label %b10

b8:
  %59 = load i16, ptr %28, !tbaa !2
  %60 = add nsw i16 %59, 1
  store i16 %60, ptr %28, !tbaa !2
  br label %b6

b9:
  br label %b4

b10:
  %61 = load i16, ptr %26, !tbaa !2
  %62 = load i16, ptr %25, !tbaa !2
  %63 = icmp slt i16 %61, %62
  br i1 %63, label %b11, label %b13

b11:
  %64 = load i16, ptr %30, !tbaa !2
  %65 = load i16, ptr %28, !tbaa !2
  %66 = load i16, ptr %26, !tbaa !2
  %67 = load i8, ptr %36, !tbaa !2
  %68 = sub i16 %64, 0
  %69 = sub i16 %65, 0
  %70 = mul i16 %68, 3
  %71 = add i16 %70, %69
  %72 = sub i16 %66, 0
  %73 = mul i16 %71, 4
  %74 = add i16 %73, %72
  %75 = getelementptr inbounds i8, ptr %35, i16 %74
  store i8 %67, ptr %75, !tbaa !2
  br label %b12

b12:
  %76 = load i16, ptr %26, !tbaa !2
  %77 = add nsw i16 %76, 1
  store i16 %77, ptr %26, !tbaa !2
  br label %b10

b13:
  br label %b8

b14:
  %78 = load i16, ptr %17, !tbaa !2
  %79 = load i16, ptr %16, !tbaa !2
  %80 = icmp slt i16 %78, %79
  br i1 %80, label %b15, label %b17

b15:
  store i16 0, ptr %15, !tbaa !2
  store i16 2, ptr %14, !tbaa !2
  br label %b18

b16:
  %81 = load i16, ptr %17, !tbaa !2
  %82 = add nsw i16 %81, 1
  store i16 %82, ptr %17, !tbaa !2
  br label %b14

b17:
  %83 = getelementptr inbounds i32, ptr %23, i16 10
  store i32 42, ptr %83, !tbaa !2
  %84 = addrspacecast ptr %40 to ptr addrspace(1)
  store i16 3, ptr %9, !tbaa !2
  %85 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 3, ptr %85, !tbaa !2
  %86 = getelementptr inbounds i8, ptr %9, i16 4
  store i16 9, ptr %86, !tbaa !2
  %87 = getelementptr inbounds i8, ptr %9, i16 6
  store ptr addrspace(1) %84, ptr %87, !tbaa !2
  %88 = addrspacecast ptr %9 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @bump(ptr addrspace(1) %88)
  store i16 0, ptr %8, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 2, ptr %6, !tbaa !2
  br label %b30

b18:
  %89 = load i16, ptr %15, !tbaa !2
  %90 = load i16, ptr %14, !tbaa !2
  %91 = icmp slt i16 %89, %90
  br i1 %91, label %b19, label %b21

b19:
  store i16 0, ptr %13, !tbaa !2
  store i16 2, ptr %12, !tbaa !2
  br label %b22

b20:
  %92 = load i16, ptr %15, !tbaa !2
  %93 = add nsw i16 %92, 1
  store i16 %93, ptr %15, !tbaa !2
  br label %b18

b21:
  br label %b16

b22:
  %94 = load i16, ptr %13, !tbaa !2
  %95 = load i16, ptr %12, !tbaa !2
  %96 = icmp slt i16 %94, %95
  br i1 %96, label %b23, label %b25

b23:
  store i16 0, ptr %11, !tbaa !2
  store i16 2, ptr %10, !tbaa !2
  br label %b26

b24:
  %97 = load i16, ptr %13, !tbaa !2
  %98 = add nsw i16 %97, 1
  store i16 %98, ptr %13, !tbaa !2
  br label %b22

b25:
  br label %b20

b26:
  %99 = load i16, ptr %11, !tbaa !2
  %100 = load i16, ptr %10, !tbaa !2
  %101 = icmp slt i16 %99, %100
  br i1 %101, label %b27, label %b29

b27:
  %102 = load i16, ptr %17, !tbaa !2
  %103 = load i16, ptr %15, !tbaa !2
  %104 = load i16, ptr %13, !tbaa !2
  %105 = load i16, ptr %11, !tbaa !2
  %106 = load i32, ptr %24, !tbaa !2
  %107 = sub i16 %102, 0
  %108 = sub i16 %103, 0
  %109 = mul i16 %107, 2
  %110 = add i16 %109, %108
  %111 = sub i16 %104, 0
  %112 = mul i16 %110, 2
  %113 = add i16 %112, %111
  %114 = sub i16 %105, 0
  %115 = mul i16 %113, 2
  %116 = add i16 %115, %114
  %117 = getelementptr inbounds i32, ptr %23, i16 %116
  store i32 %106, ptr %117, !tbaa !2
  br label %b28

b28:
  %118 = load i16, ptr %11, !tbaa !2
  %119 = add nsw i16 %118, 1
  store i16 %119, ptr %11, !tbaa !2
  br label %b26

b29:
  br label %b24

b30:
  %120 = load i16, ptr %7, !tbaa !2
  %121 = load i16, ptr %6, !tbaa !2
  %122 = icmp slt i16 %120, %121
  br i1 %122, label %b31, label %b33

b31:
  store i16 0, ptr %5, !tbaa !2
  store i16 3, ptr %4, !tbaa !2
  br label %b34

b32:
  %123 = load i16, ptr %7, !tbaa !2
  %124 = add nsw i16 %123, 1
  store i16 %124, ptr %7, !tbaa !2
  br label %b30

b33:
  %125 = addrspacecast ptr %40 to ptr addrspace(1)
  store i16 3, ptr %1, !tbaa !2
  %126 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 3, ptr %126, !tbaa !2
  %127 = getelementptr inbounds i8, ptr %1, i16 4
  store i16 9, ptr %127, !tbaa !2
  %128 = getelementptr inbounds i8, ptr %1, i16 6
  store ptr addrspace(1) %125, ptr %128, !tbaa !2
  %129 = addrspacecast ptr %1 to ptr addrspace(1)
  %130 = call cc 1005 addrspace(1) i16 @trace(ptr addrspace(1) %129)
  store i16 %130, ptr %0, !tbaa !2
  %131 = load i16, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %131)
  %132 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %132)
  %133 = getelementptr inbounds i16, ptr %40, i16 5
  %134 = load i16, ptr %133, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %134)
  %135 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %135)
  %136 = load i16, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %136)
  %137 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %137)
  %138 = getelementptr inbounds i32, ptr %23, i16 10
  %139 = load i32, ptr %138, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI4(i32 %139)
  %140 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %140)
  call cc 1005 addrspace(1) void @N$PU2(i16 9)
  %141 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %141)
  call cc 1005 addrspace(1) void @N$PU2(i16 3)
  %142 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %142)
  call cc 1005 addrspace(1) void @N$PU2(i16 4)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0

b34:
  %143 = load i16, ptr %5, !tbaa !2
  %144 = load i16, ptr %4, !tbaa !2
  %145 = icmp slt i16 %143, %144
  br i1 %145, label %b35, label %b37

b35:
  store i16 0, ptr %3, !tbaa !2
  store i16 4, ptr %2, !tbaa !2
  br label %b38

b36:
  %146 = load i16, ptr %5, !tbaa !2
  %147 = add nsw i16 %146, 1
  store i16 %147, ptr %5, !tbaa !2
  br label %b34

b37:
  br label %b32

b38:
  %148 = load i16, ptr %3, !tbaa !2
  %149 = load i16, ptr %2, !tbaa !2
  %150 = icmp slt i16 %148, %149
  br i1 %150, label %b39, label %b41

b39:
  %151 = load i16, ptr %8, !tbaa !2
  %152 = load i16, ptr %7, !tbaa !2
  %153 = load i16, ptr %5, !tbaa !2
  %154 = load i16, ptr %3, !tbaa !2
  %155 = icmp ult i16 %152, 2
  br i1 %155, label %b42, label %b43

b40:
  %156 = load i16, ptr %3, !tbaa !2
  %157 = add nsw i16 %156, 1
  store i16 %157, ptr %3, !tbaa !2
  br label %b38

b41:
  br label %b36

b42:
  %158 = icmp ult i16 %153, 3
  br i1 %158, label %b44, label %b45

b43:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b44:
  %159 = icmp ult i16 %154, 4
  br i1 %159, label %b46, label %b47

b45:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b46:
  %160 = sub i16 %152, 0
  %161 = sub i16 %153, 0
  %162 = mul i16 %160, 3
  %163 = add i16 %162, %161
  %164 = sub i16 %154, 0
  %165 = mul i16 %163, 4
  %166 = add i16 %165, %164
  %167 = getelementptr inbounds i8, ptr %35, i16 %166
  %168 = load i8, ptr %167, !tbaa !2
  %169 = zext i8 %168 to i16
  %170 = add i16 %151, %169
  store i16 %170, ptr %8, !tbaa !2
  br label %b40

b47:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 -32768}
!6 = !{i16 0, i16 -32768}
!7 = !{i16 0, i16 -32768}
!8 = !{i16 0, i16 -32768}
