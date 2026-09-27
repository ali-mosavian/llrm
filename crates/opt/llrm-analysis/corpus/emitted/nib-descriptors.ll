target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [12 x i8] c"\08\00\05\00\05\00metal\00"
@$str2 = internal constant [22 x i8] c"\08\00\0F\00\0F\00descriptors: ok\00"
@$str3 = internal constant [23 x i8] c"\08\00\10\00\10\00descriptors: bad\00"

define internal i16 @sum(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %2, !tbaa !2
  %3 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = icmp ult i16 %4, %3
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b3, label %b5

b3:
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %9 = load ptr addrspace(1), ptr addrspace(1) %8
  %10 = mul i16 %4, 2
  %11 = getelementptr i8, ptr addrspace(1) %9, i16 %10
  %12 = load i16, ptr %2, !tbaa !2
  %13 = load i16, ptr addrspace(1) %11
  %14 = add i16 %12, %13
  store i16 %14, ptr %2, !tbaa !2
  br label %b4

b4:
  %15 = load i16, ptr %1, !tbaa !2
  %16 = add i16 %15, 1
  store i16 %16, ptr %1, !tbaa !2
  br label %b2

b5:
  %17 = load i16, ptr %2, !tbaa !2
  ret i16 %17
}

define internal i8 @first(ptr %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca ptr
  store i16 0, ptr %1
  store ptr null, ptr %2
  store ptr %0, ptr %2, !tbaa !2
  %3 = load ptr, ptr %2, !tbaa !2
  %4 = getelementptr i8, ptr %3, i16 -4
  %5 = load i16, ptr %4
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %1, !tbaa !2
  %7 = icmp ult i16 %6, %5
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b3, label %b5

b3:
  %10 = getelementptr i8, ptr %3, i16 %6
  %11 = load i8, ptr %10
  %12 = load ptr, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %12)
  ret i8 %11

b5:
  %13 = load ptr, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %13)
  ret i8 0
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca [8 x i8]
  %2 = alloca ptr
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [10 x i8]
  store i16 0, ptr %0
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  store ptr null, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 10, i1 false)
  store i16 5, ptr %3, !tbaa !2
  store i16 5, ptr %4, !tbaa !2
  %6 = sub i16 0, 0
  %7 = getelementptr inbounds i16, ptr %5, i16 %6
  store i16 1, ptr %7, !tbaa !2
  %8 = sub i16 1, 0
  %9 = getelementptr inbounds i16, ptr %5, i16 %8
  store i16 2, ptr %9, !tbaa !2
  %10 = sub i16 2, 0
  %11 = getelementptr inbounds i16, ptr %5, i16 %10
  store i16 4, ptr %11, !tbaa !2
  %12 = sub i16 3, 0
  %13 = getelementptr inbounds i16, ptr %5, i16 %12
  store i16 8, ptr %13, !tbaa !2
  %14 = sub i16 4, 0
  %15 = getelementptr inbounds i16, ptr %5, i16 %14
  store i16 16, ptr %15, !tbaa !2
  %16 = getelementptr i8, ptr @$str1, i16 6
  store ptr %16, ptr %2, !tbaa !2
  %17 = addrspacecast ptr %5 to ptr addrspace(1)
  %18 = getelementptr i8, ptr addrspace(1) %17, i16 2
  store i16 3, ptr %1, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 3, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %18, ptr %20, !tbaa !2
  %21 = addrspacecast ptr %1 to ptr addrspace(1)
  %22 = call addrspace(1) i16 @sum(ptr addrspace(1) %21)
  store i16 %22, ptr %0, !tbaa !2
  %23 = icmp eq i16 5, 5
  %24 = sext i1 %23 to i8
  %25 = icmp ne i8 %24, 0
  br i1 %25, label %b2, label %b3

b2:
  %26 = icmp eq i16 5, 5
  %27 = sext i1 %26 to i8
  %28 = icmp ne i8 %27, 0
  br i1 %28, label %b5, label %b6

b3:
  br label %b4

b4:
  %29 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %29)
  call addrspace(1) void @N$PN()
  %30 = load ptr, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %30)
  ret i16 1

b5:
  %31 = load i16, ptr %0, !tbaa !2
  %32 = icmp eq i16 %31, 14
  %33 = sext i1 %32 to i8
  %34 = icmp ne i8 %33, 0
  br i1 %34, label %b8, label %b9

b6:
  br label %b7

b7:
  br label %b4

b8:
  %35 = load ptr, ptr %2, !tbaa !2
  %36 = getelementptr i8, ptr %35, i16 -4
  %37 = load i16, ptr %36
  %38 = icmp eq i16 %37, 5
  %39 = sext i1 %38 to i8
  %40 = icmp ne i8 %39, 0
  br i1 %40, label %b11, label %b12

b9:
  br label %b10

b10:
  br label %b7

b11:
  %41 = load ptr, ptr %2, !tbaa !2
  %42 = getelementptr i8, ptr %41, i16 -2
  %43 = load i16, ptr %42
  %44 = icmp eq i16 %43, 5
  %45 = sext i1 %44 to i8
  %46 = icmp ne i8 %45, 0
  br i1 %46, label %b14, label %b15

b12:
  br label %b13

b13:
  br label %b10

b14:
  %47 = load ptr, ptr %2, !tbaa !2
  store ptr null, ptr %2, !tbaa !2
  %48 = call addrspace(1) i8 @first(ptr %47)
  %49 = icmp eq i8 %48, 109
  %50 = sext i1 %49 to i8
  %51 = icmp ne i8 %50, 0
  br i1 %51, label %b17, label %b18

b15:
  br label %b16

b16:
  br label %b13

b17:
  %52 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %52)
  call addrspace(1) void @N$PN()
  %53 = load ptr, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %53)
  ret i16 0

b18:
  br label %b19

b19:
  br label %b16
}

declare void @N$BDRP(ptr) addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
