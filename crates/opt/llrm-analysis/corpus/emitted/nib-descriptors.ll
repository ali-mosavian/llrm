target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [12 x i8] c"\08\00\05\00\05\00metal\00"
@$str2 = internal constant [22 x i8] c"\08\00\0F\00\0F\00descriptors: ok\00"
@$str3 = internal constant [23 x i8] c"\08\00\10\00\10\00descriptors: bad\00"

define internal cc 1005 i16 @sum(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 0, ptr %2, !tbaa !2
  %3 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = icmp ult i16 %4, %3
  br i1 %5, label %b3, label %b5

b3:
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %7 = load ptr addrspace(1), ptr addrspace(1) %6
  %8 = mul i16 %4, 2
  %9 = getelementptr inbounds i8, ptr addrspace(1) %7, i16 %8
  %10 = load i16, ptr %2, !tbaa !2
  %11 = load i16, ptr addrspace(1) %9
  %12 = add i16 %10, %11
  store i16 %12, ptr %2, !tbaa !2
  br label %b4

b4:
  %13 = load i16, ptr %1, !tbaa !2
  %14 = add i16 %13, 1
  store i16 %14, ptr %1, !tbaa !2
  br label %b2

b5:
  %15 = load i16, ptr %2, !tbaa !2
  ret i16 %15
}

define internal cc 1005 i8 @first(ptr %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca ptr
  store ptr %0, ptr %2, !tbaa !2
  %3 = load ptr, ptr %2, !tbaa !2
  %4 = getelementptr i8, ptr %3, i16 -4
  %5 = load i16, ptr %4
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %1, !tbaa !2
  %7 = icmp ult i16 %6, %5
  br i1 %7, label %b3, label %b5

b3:
  %8 = getelementptr inbounds i8, ptr %3, i16 %6
  %9 = load i8, ptr %8
  %10 = load ptr, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %10)
  ret i8 %9

b5:
  %11 = load ptr, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %11)
  ret i8 0
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca [8 x i8]
  %2 = alloca ptr
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [10 x i8]
  store i16 5, ptr %3, !tbaa !2
  store i16 5, ptr %4, !tbaa !2
  %6 = getelementptr inbounds i16, ptr %5, i16 0
  store i16 1, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i16, ptr %5, i16 1
  store i16 2, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i16, ptr %5, i16 2
  store i16 4, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i16, ptr %5, i16 3
  store i16 8, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i16, ptr %5, i16 4
  store i16 16, ptr %10, !tbaa !2
  %11 = getelementptr i8, ptr @$str1, i16 6
  store ptr %11, ptr %2, !tbaa !2
  %12 = addrspacecast ptr %5 to ptr addrspace(1)
  %13 = getelementptr inbounds i8, ptr addrspace(1) %12, i16 2
  store i16 3, ptr %1, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 3, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %13, ptr %15, !tbaa !2
  %16 = addrspacecast ptr %1 to ptr addrspace(1)
  %17 = call cc 1005 addrspace(1) i16 @sum(ptr addrspace(1) %16)
  store i16 %17, ptr %0, !tbaa !2
  br i1 true, label %b2, label %b3

b2:
  br i1 true, label %b5, label %b6

b3:
  br label %b4

b4:
  %18 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %18)
  call cc 1005 addrspace(1) void @N$PN()
  %19 = load ptr, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %19)
  ret i16 1

b5:
  %20 = load i16, ptr %0, !tbaa !2
  %21 = icmp eq i16 %20, 14
  br i1 %21, label %b8, label %b9

b6:
  br label %b7

b7:
  br label %b4

b8:
  %22 = load ptr, ptr %2, !tbaa !2
  %23 = getelementptr i8, ptr %22, i16 -4
  %24 = load i16, ptr %23
  %25 = icmp eq i16 %24, 5
  br i1 %25, label %b11, label %b12

b9:
  br label %b10

b10:
  br label %b7

b11:
  %26 = load ptr, ptr %2, !tbaa !2
  %27 = getelementptr i8, ptr %26, i16 -2
  %28 = load i16, ptr %27
  %29 = icmp eq i16 %28, 5
  br i1 %29, label %b14, label %b15

b12:
  br label %b13

b13:
  br label %b10

b14:
  %30 = load ptr, ptr %2, !tbaa !2
  store ptr null, ptr %2, !tbaa !2
  %31 = call cc 1005 addrspace(1) i8 @first(ptr %30)
  %32 = icmp eq i8 %31, 109
  br i1 %32, label %b17, label %b18

b15:
  br label %b16

b16:
  br label %b13

b17:
  %33 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %33)
  call cc 1005 addrspace(1) void @N$PN()
  %34 = load ptr, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %34)
  ret i16 0

b18:
  br label %b19

b19:
  br label %b16
}

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
