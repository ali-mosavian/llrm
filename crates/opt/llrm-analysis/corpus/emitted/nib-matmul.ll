target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [15 x i8] c"\08\00\08\00\08\00matmul: \00"
@$str2 = internal constant [19 x i8] c"\08\00\0C\00\0C\00matmul: bad \00"

define internal cc 1005 i32 @matmul(i32 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i32
  %2 = alloca i32
  %3 = alloca i32
  %4 = alloca i32
  %5 = alloca i32
  %6 = alloca i32
  %7 = alloca i32
  %8 = alloca i32
  %9 = alloca i32
  %10 = alloca i32
  %11 = alloca i32
  %12 = alloca i32
  %13 = alloca i32
  %14 = alloca i32
  %15 = alloca i32
  %16 = alloca i32
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca [256 x i8]
  %22 = alloca i32
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca [256 x i8]
  %28 = alloca i32
  %29 = alloca i16
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca [256 x i8]
  %34 = alloca i32
  %35 = alloca i32
  store i32 8, ptr %35, !tbaa !2
  store i32 0, ptr %34, !tbaa !2
  store i16 64, ptr %31, !tbaa !2
  store i16 64, ptr %32, !tbaa !2
  store i16 0, ptr %30, !tbaa !2
  store i16 64, ptr %29, !tbaa !2
  br label %b2

b2:
  %36 = load i16, ptr %30, !tbaa !2
  %37 = load i16, ptr %29, !tbaa !2
  %38 = icmp slt i16 %36, %37
  br i1 %38, label %b3, label %b5

b3:
  %39 = load i16, ptr %30, !tbaa !2
  %40 = load i32, ptr %34, !tbaa !2
  %41 = sub i16 %39, 0
  %42 = getelementptr inbounds i32, ptr %33, i16 %41
  store i32 %40, ptr %42, !tbaa !2
  br label %b4

b4:
  %43 = load i16, ptr %30, !tbaa !2
  %44 = add nsw i16 %43, 1
  store i16 %44, ptr %30, !tbaa !2
  br label %b2

b5:
  store i32 0, ptr %28, !tbaa !2
  store i16 64, ptr %25, !tbaa !2
  store i16 64, ptr %26, !tbaa !2
  store i16 0, ptr %24, !tbaa !2
  store i16 64, ptr %23, !tbaa !2
  br label %b6

b6:
  %45 = load i16, ptr %24, !tbaa !2
  %46 = load i16, ptr %23, !tbaa !2
  %47 = icmp slt i16 %45, %46
  br i1 %47, label %b7, label %b9

b7:
  %48 = load i16, ptr %24, !tbaa !2
  %49 = load i32, ptr %28, !tbaa !2
  %50 = sub i16 %48, 0
  %51 = getelementptr inbounds i32, ptr %27, i16 %50
  store i32 %49, ptr %51, !tbaa !2
  br label %b8

b8:
  %52 = load i16, ptr %24, !tbaa !2
  %53 = add nsw i16 %52, 1
  store i16 %53, ptr %24, !tbaa !2
  br label %b6

b9:
  store i32 0, ptr %22, !tbaa !2
  store i16 64, ptr %19, !tbaa !2
  store i16 64, ptr %20, !tbaa !2
  store i16 0, ptr %18, !tbaa !2
  store i16 64, ptr %17, !tbaa !2
  br label %b10

b10:
  %54 = load i16, ptr %18, !tbaa !2
  %55 = load i16, ptr %17, !tbaa !2
  %56 = icmp slt i16 %54, %55
  br i1 %56, label %b11, label %b13

b11:
  %57 = load i16, ptr %18, !tbaa !2
  %58 = load i32, ptr %22, !tbaa !2
  %59 = sub i16 %57, 0
  %60 = getelementptr inbounds i32, ptr %21, i16 %59
  store i32 %58, ptr %60, !tbaa !2
  br label %b12

b12:
  %61 = load i16, ptr %18, !tbaa !2
  %62 = add nsw i16 %61, 1
  store i16 %62, ptr %18, !tbaa !2
  br label %b10

b13:
  %63 = load i32, ptr %35, !tbaa !2
  store i32 0, ptr %16, !tbaa !2
  store i32 %63, ptr %15, !tbaa !2
  br label %b14

b14:
  %64 = load i32, ptr %16, !tbaa !2
  %65 = load i32, ptr %15, !tbaa !2
  %66 = icmp slt i32 %64, %65
  br i1 %66, label %b15, label %b17

b15:
  %67 = load i32, ptr %35, !tbaa !2
  store i32 0, ptr %14, !tbaa !2
  store i32 %67, ptr %13, !tbaa !2
  br label %b18

b16:
  %68 = load i32, ptr %16, !tbaa !2
  %69 = add nsw i32 %68, 1
  store i32 %69, ptr %16, !tbaa !2
  br label %b14

b17:
  %70 = load i32, ptr %35, !tbaa !2
  store i32 0, ptr %12, !tbaa !2
  store i32 %70, ptr %11, !tbaa !2
  br label %b31

b18:
  %71 = load i32, ptr %14, !tbaa !2
  %72 = load i32, ptr %13, !tbaa !2
  %73 = icmp slt i32 %71, %72
  br i1 %73, label %b19, label %b21

b19:
  %74 = load i32, ptr %16, !tbaa !2
  %75 = mul i32 %74, 8
  %76 = load i32, ptr %14, !tbaa !2
  %77 = add i32 %75, %76
  %78 = icmp ult i32 %77, 64
  br i1 %78, label %b22, label %b23

b20:
  %79 = load i32, ptr %14, !tbaa !2
  %80 = add nsw i32 %79, 1
  store i32 %80, ptr %14, !tbaa !2
  br label %b18

b21:
  br label %b16

b22:
  %81 = load i32, ptr %16, !tbaa !2
  %82 = mul i32 %81, 3
  %83 = load i32, ptr %14, !tbaa !2
  %84 = add i32 %82, %83
  %85 = add i32 %84, 1
  %86 = add i32 %85, %0
  %87 = trunc i32 %77 to i16
  %88 = sub i16 %87, 0
  %89 = getelementptr inbounds i32, ptr %33, i16 %88
  store i32 %86, ptr %89, !tbaa !2
  %90 = load i32, ptr %16, !tbaa !2
  %91 = load i32, ptr %14, !tbaa !2
  %92 = icmp eq i32 %90, %91
  br i1 %92, label %b24, label %b25

b23:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b24:
  %93 = load i32, ptr %16, !tbaa !2
  %94 = mul i32 %93, 8
  %95 = load i32, ptr %14, !tbaa !2
  %96 = add i32 %94, %95
  %97 = icmp ult i32 %96, 64
  br i1 %97, label %b27, label %b28

b25:
  %98 = load i32, ptr %16, !tbaa !2
  %99 = mul i32 %98, 8
  %100 = load i32, ptr %14, !tbaa !2
  %101 = add i32 %99, %100
  %102 = icmp ult i32 %101, 64
  br i1 %102, label %b29, label %b30

b26:
  br label %b20

b27:
  %103 = trunc i32 %96 to i16
  %104 = sub i16 %103, 0
  %105 = getelementptr inbounds i32, ptr %27, i16 %104
  store i32 2, ptr %105, !tbaa !2
  br label %b26

b28:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b29:
  %106 = load i32, ptr %16, !tbaa !2
  %107 = load i32, ptr %14, !tbaa !2
  %108 = add i32 %106, %107
  %109 = srem i32 %108, 3
  %110 = trunc i32 %101 to i16
  %111 = sub i16 %110, 0
  %112 = getelementptr inbounds i32, ptr %27, i16 %111
  store i32 %109, ptr %112, !tbaa !2
  br label %b26

b30:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b31:
  %113 = load i32, ptr %12, !tbaa !2
  %114 = load i32, ptr %11, !tbaa !2
  %115 = icmp slt i32 %113, %114
  br i1 %115, label %b32, label %b34

b32:
  %116 = load i32, ptr %35, !tbaa !2
  store i32 0, ptr %10, !tbaa !2
  store i32 %116, ptr %9, !tbaa !2
  br label %b35

b33:
  %117 = load i32, ptr %12, !tbaa !2
  %118 = add nsw i32 %117, 1
  store i32 %118, ptr %12, !tbaa !2
  br label %b31

b34:
  store i32 0, ptr %5, !tbaa !2
  %119 = load i32, ptr %35, !tbaa !2
  store i32 0, ptr %4, !tbaa !2
  store i32 %119, ptr %3, !tbaa !2
  br label %b49

b35:
  %120 = load i32, ptr %10, !tbaa !2
  %121 = load i32, ptr %9, !tbaa !2
  %122 = icmp slt i32 %120, %121
  br i1 %122, label %b36, label %b38

b36:
  store i32 0, ptr %8, !tbaa !2
  %123 = load i32, ptr %35, !tbaa !2
  store i32 0, ptr %7, !tbaa !2
  store i32 %123, ptr %6, !tbaa !2
  br label %b39

b37:
  %124 = load i32, ptr %10, !tbaa !2
  %125 = add nsw i32 %124, 1
  store i32 %125, ptr %10, !tbaa !2
  br label %b35

b38:
  br label %b33

b39:
  %126 = load i32, ptr %7, !tbaa !2
  %127 = load i32, ptr %6, !tbaa !2
  %128 = icmp slt i32 %126, %127
  br i1 %128, label %b40, label %b42

b40:
  %129 = load i32, ptr %8, !tbaa !2
  %130 = load i32, ptr %12, !tbaa !2
  %131 = mul i32 %130, 8
  %132 = load i32, ptr %7, !tbaa !2
  %133 = add i32 %131, %132
  %134 = icmp ult i32 %133, 64
  br i1 %134, label %b43, label %b44

b41:
  %135 = load i32, ptr %7, !tbaa !2
  %136 = add nsw i32 %135, 1
  store i32 %136, ptr %7, !tbaa !2
  br label %b39

b42:
  %137 = load i32, ptr %12, !tbaa !2
  %138 = mul i32 %137, 8
  %139 = load i32, ptr %10, !tbaa !2
  %140 = add i32 %138, %139
  %141 = icmp ult i32 %140, 64
  br i1 %141, label %b47, label %b48

b43:
  %142 = trunc i32 %133 to i16
  %143 = sub i16 %142, 0
  %144 = getelementptr inbounds i32, ptr %33, i16 %143
  %145 = load i32, ptr %144, !tbaa !2
  %146 = load i32, ptr %7, !tbaa !2
  %147 = mul i32 %146, 8
  %148 = load i32, ptr %10, !tbaa !2
  %149 = add i32 %147, %148
  %150 = icmp ult i32 %149, 64
  br i1 %150, label %b45, label %b46

b44:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b45:
  %151 = trunc i32 %149 to i16
  %152 = sub i16 %151, 0
  %153 = getelementptr inbounds i32, ptr %27, i16 %152
  %154 = load i32, ptr %153, !tbaa !2
  %155 = mul i32 %145, %154
  %156 = add i32 %129, %155
  store i32 %156, ptr %8, !tbaa !2
  br label %b41

b46:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b47:
  %157 = load i32, ptr %8, !tbaa !2
  %158 = trunc i32 %140 to i16
  %159 = sub i16 %158, 0
  %160 = getelementptr inbounds i32, ptr %21, i16 %159
  store i32 %157, ptr %160, !tbaa !2
  br label %b37

b48:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b49:
  %161 = load i32, ptr %4, !tbaa !2
  %162 = load i32, ptr %3, !tbaa !2
  %163 = icmp slt i32 %161, %162
  br i1 %163, label %b50, label %b52

b50:
  %164 = load i32, ptr %35, !tbaa !2
  store i32 0, ptr %2, !tbaa !2
  store i32 %164, ptr %1, !tbaa !2
  br label %b53

b51:
  %165 = load i32, ptr %4, !tbaa !2
  %166 = add nsw i32 %165, 1
  store i32 %166, ptr %4, !tbaa !2
  br label %b49

b52:
  %167 = load i32, ptr %5, !tbaa !2
  ret i32 %167

b53:
  %168 = load i32, ptr %2, !tbaa !2
  %169 = load i32, ptr %1, !tbaa !2
  %170 = icmp slt i32 %168, %169
  br i1 %170, label %b54, label %b56

b54:
  %171 = load i32, ptr %5, !tbaa !2
  %172 = load i32, ptr %4, !tbaa !2
  %173 = mul i32 %172, 8
  %174 = load i32, ptr %2, !tbaa !2
  %175 = add i32 %173, %174
  %176 = icmp ult i32 %175, 64
  br i1 %176, label %b57, label %b58

b55:
  %177 = load i32, ptr %2, !tbaa !2
  %178 = add nsw i32 %177, 1
  store i32 %178, ptr %2, !tbaa !2
  br label %b53

b56:
  br label %b51

b57:
  %179 = trunc i32 %175 to i16
  %180 = sub i16 %179, 0
  %181 = getelementptr inbounds i32, ptr %21, i16 %180
  %182 = load i32, ptr %181, !tbaa !2
  %183 = load i32, ptr %4, !tbaa !2
  %184 = mul i32 %183, 8
  %185 = load i32, ptr %2, !tbaa !2
  %186 = add i32 %184, %185
  %187 = add i32 %186, 1
  %188 = mul i32 %182, %187
  %189 = add i32 %171, %188
  store i32 %189, ptr %5, !tbaa !2
  br label %b55

b58:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i32
  %1 = call cc 1005 addrspace(1) i32 @matmul(i32 1)
  store i32 %1, ptr %0, !tbaa !2
  %2 = load i32, ptr %0, !tbaa !2
  %3 = icmp eq i32 %2, 372432
  br i1 %3, label %b2, label %b3

b2:
  %4 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %4)
  %5 = load i32, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI4(i32 %5)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0

b3:
  br label %b4

b4:
  %6 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %6)
  %7 = load i32, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI4(i32 %7)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 1
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
