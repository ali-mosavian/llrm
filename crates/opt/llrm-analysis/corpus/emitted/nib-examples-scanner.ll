target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [28 x i8] c"\08\00\15\00\15\00move  north then east\00"
@$str2 = internal constant [8 x i8] c"\08\00\01\00\01\00[\00"
@$str3 = internal constant [8 x i8] c"\08\00\01\00\01\00]\00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00 words, \00"
@$str5 = internal constant [22 x i8] c"\08\00\0F\00\0F\00 letters, head \00"

define internal cc 1005 i16 @run(ptr addrspace(1) nonnull dereferenceable(2) readonly noalias nocapture %0, i16 %1, i8 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i16
  store i16 %1, ptr %3, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %3, !tbaa !2
  %5 = load ptr, ptr addrspace(1) %0
  %6 = getelementptr i8, ptr %5, i16 -4
  %7 = load i16, ptr %6
  %8 = icmp ult i16 %4, %7
  br i1 %8, label %b5, label %b4

b3:
  %9 = load i16, ptr %3, !tbaa !2
  %10 = add i16 %9, 1
  store i16 %10, ptr %3, !tbaa !2
  br label %b2

b4:
  %11 = load i16, ptr %3, !tbaa !2
  ret i16 %11

b5:
  %12 = load ptr, ptr addrspace(1) %0
  %13 = load i16, ptr %3, !tbaa !2
  %14 = getelementptr i8, ptr %12, i16 -4
  %15 = load i16, ptr %14
  %16 = icmp ult i16 %13, %15
  br i1 %16, label %b6, label %b7

b6:
  %17 = getelementptr inbounds i8, ptr %12, i16 %13
  %18 = load i8, ptr %17
  %19 = icmp eq i8 %18, 32
  %20 = zext i1 %19 to i8
  %21 = icmp eq i8 %20, %2
  br i1 %21, label %b3, label %b4

b7:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 void @view(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias %1, i16 %2, i16 %3) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %4 = alloca [8 x i8]
  %5 = load ptr, ptr addrspace(1) %1
  %6 = getelementptr i8, ptr %5, i16 -4
  %7 = load i16, ptr %6
  %8 = addrspacecast ptr %5 to ptr addrspace(1)
  %9 = icmp ule i16 %3, %7
  br i1 %9, label %b2, label %b3

b2:
  %10 = icmp ule i16 %2, %3
  br i1 %10, label %b4, label %b5

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b4:
  %11 = getelementptr inbounds i8, ptr addrspace(1) %8, i16 %2
  %12 = sub i16 %3, %2
  store i16 %12, ptr %4, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %12, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %11, ptr %14, !tbaa !2
  %15 = addrspacecast ptr %4 to ptr addrspace(1)
  %16 = load i16, ptr addrspace(1) %15, !tbaa !2
  store i16 %16, ptr addrspace(1) %0
  %17 = getelementptr i8, ptr addrspace(1) %15, i16 2
  %18 = load i16, ptr addrspace(1) %17, !tbaa !2
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %18, ptr addrspace(1) %19
  %20 = getelementptr i8, ptr addrspace(1) %15, i16 4
  %21 = load ptr addrspace(1), ptr addrspace(1) %20, !tbaa !2
  %22 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %21, ptr addrspace(1) %22
  ret void

b5:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 void @scan(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  store ptr addrspace(1) %1, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 0, ptr addrspace(1) %2
  ret void
}

define internal cc 1005 void @skip(ptr addrspace(1) %0, ptr addrspace(1) noalias %1, i16 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca [6 x i8]
  %4 = load ptr addrspace(1), ptr addrspace(1) %1
  %5 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %6 = load i16, ptr addrspace(1) %5
  store ptr addrspace(1) %4, ptr %3, !tbaa !2
  %7 = getelementptr inbounds i8, ptr %3, i16 4
  store i16 %6, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %3, i16 4
  %9 = load i16, ptr %8, !tbaa !2
  %10 = add i16 %9, %2
  %11 = getelementptr inbounds i8, ptr %3, i16 4
  store i16 %10, ptr %11, !tbaa !2
  %12 = load ptr addrspace(1), ptr %3, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %3, i16 4
  %14 = load i16, ptr %13, !tbaa !2
  store ptr addrspace(1) %12, ptr addrspace(1) %0
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %14, ptr addrspace(1) %15
  ret void
}

define internal cc 1005 void @Scanner.word(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(6) noalias %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca [8 x i8]
  %3 = alloca i16
  %4 = load ptr addrspace(1), ptr addrspace(1) %1
  %5 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %6 = load i16, ptr addrspace(1) %5
  %7 = call cc 1005 addrspace(1) i16 @run(ptr addrspace(1) %4, i16 %6, i8 1)
  store i16 %7, ptr %3, !tbaa !2
  %8 = load ptr addrspace(1), ptr addrspace(1) %1
  %9 = load i16, ptr %3, !tbaa !2
  %10 = call cc 1005 addrspace(1) i16 @run(ptr addrspace(1) %8, i16 %9, i8 0)
  %11 = getelementptr i8, ptr addrspace(1) %1, i16 4
  store i16 %10, ptr addrspace(1) %11
  %12 = addrspacecast ptr %2 to ptr addrspace(1)
  %13 = load ptr addrspace(1), ptr addrspace(1) %1
  %14 = load i16, ptr %3, !tbaa !2
  %15 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %16 = load i16, ptr addrspace(1) %15
  call cc 1005 addrspace(1) void @view(ptr addrspace(1) %12, ptr addrspace(1) %13, i16 %14, i16 %16)
  %17 = load i16, ptr addrspace(1) %12, !tbaa !2
  store i16 %17, ptr addrspace(1) %0
  %18 = getelementptr i8, ptr addrspace(1) %12, i16 2
  %19 = load i16, ptr addrspace(1) %18, !tbaa !2
  %20 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %19, ptr addrspace(1) %20
  %21 = getelementptr i8, ptr addrspace(1) %12, i16 4
  %22 = load ptr addrspace(1), ptr addrspace(1) %21, !tbaa !2
  %23 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %22, ptr addrspace(1) %23
  ret void
}

define internal cc 1005 void @count(ptr addrspace(1) nonnull dereferenceable(4) noalias nocapture %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = load i16, ptr addrspace(1) %0
  %3 = add i16 %2, 1
  store i16 %3, ptr addrspace(1) %0
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %5 = load i16, ptr addrspace(1) %4
  %6 = load i16, ptr addrspace(1) %1
  %7 = add i16 %5, %6
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %7, ptr addrspace(1) %8
  ret void
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [6 x i8]
  %4 = alloca [6 x i8]
  %5 = alloca [6 x i8]
  %6 = alloca [6 x i8]
  %7 = alloca [4 x i8]
  %8 = alloca [8 x i8]
  %9 = alloca [2 x i8]
  %10 = getelementptr i8, ptr @$str1, i16 6
  store ptr %10, ptr %9, !tbaa !2
  %11 = addrspacecast ptr %8 to ptr addrspace(1)
  %12 = addrspacecast ptr %9 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @view(ptr addrspace(1) %11, ptr addrspace(1) %12, i16 0, i16 4)
  store i16 0, ptr %7, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 0, ptr %13, !tbaa !2
  %14 = addrspacecast ptr %5 to ptr addrspace(1)
  %15 = addrspacecast ptr %3 to ptr addrspace(1)
  %16 = addrspacecast ptr %9 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @scan(ptr addrspace(1) %15, ptr addrspace(1) %16)
  %17 = load ptr addrspace(1), ptr %3, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %3, i16 4
  %19 = load i16, ptr %18, !tbaa !2
  store ptr addrspace(1) %17, ptr %4, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %4, i16 4
  store i16 %19, ptr %20, !tbaa !2
  %21 = addrspacecast ptr %4 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @skip(ptr addrspace(1) %14, ptr addrspace(1) %21, i16 4)
  %22 = load ptr addrspace(1), ptr %5, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %5, i16 4
  %24 = load i16, ptr %23, !tbaa !2
  store ptr addrspace(1) %22, ptr %6, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %6, i16 4
  store i16 %24, ptr %25, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 3, ptr %1, !tbaa !2
  br label %b2

b2:
  %26 = load i16, ptr %2, !tbaa !2
  %27 = load i16, ptr %1, !tbaa !2
  %28 = icmp slt i16 %26, %27
  br i1 %28, label %b3, label %b5

b3:
  %29 = addrspacecast ptr %0 to ptr addrspace(1)
  %30 = addrspacecast ptr %6 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Scanner.word(ptr addrspace(1) %29, ptr addrspace(1) %30)
  %31 = addrspacecast ptr %7 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @count(ptr addrspace(1) %31, ptr addrspace(1) %29)
  %32 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %32)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %29)
  %33 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %33)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b4

b4:
  %34 = load i16, ptr %2, !tbaa !2
  %35 = add nsw i16 %34, 1
  store i16 %35, ptr %2, !tbaa !2
  br label %b2

b5:
  %36 = addrspacecast ptr %7 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @count(ptr addrspace(1) %36, ptr addrspace(1) %11)
  %37 = load i16, ptr %7, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %37)
  %38 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %38)
  %39 = getelementptr inbounds i8, ptr %7, i16 2
  %40 = load i16, ptr %39, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %40)
  %41 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %41)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %11)
  call cc 1005 addrspace(1) void @N$PN()
  %42 = load ptr, ptr %9, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %42)
  ret i16 0
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
