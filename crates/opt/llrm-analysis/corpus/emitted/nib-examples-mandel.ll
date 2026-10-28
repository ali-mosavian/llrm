target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [17 x i8] c"\08\00\0A\00\0A\00 .:-=+*#%@\00"
@$str2 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str3 = internal constant [12 x i8] c"\08\00\05\00\05\00step \00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00, limit \00"

define internal cc 1005 i16 @escape(i32 %0, i32 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i32
  %3 = alloca i32
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i32
  %7 = alloca i32
  store i32 0, ptr %7, !tbaa !2
  store i32 0, ptr %6, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  store i16 40, ptr %4, !tbaa !2
  br label %b2

b2:
  %8 = load i16, ptr %5, !tbaa !2
  %9 = load i16, ptr %4, !tbaa !2
  %10 = icmp ult i16 %8, %9
  br i1 %10, label %b3, label %b5

b3:
  %11 = load i32, ptr %7, !tbaa !2
  %12 = load i32, ptr %7, !tbaa !2
  %13 = call i32 @llvm.smul.fix.i32(i32 %11, i32 %12, i32 12)
  store i32 %13, ptr %3, !tbaa !2
  %14 = load i32, ptr %6, !tbaa !2
  %15 = load i32, ptr %6, !tbaa !2
  %16 = call i32 @llvm.smul.fix.i32(i32 %14, i32 %15, i32 12)
  store i32 %16, ptr %2, !tbaa !2
  %17 = load i32, ptr %3, !tbaa !2
  %18 = load i32, ptr %2, !tbaa !2
  %19 = add i32 %17, %18
  %20 = icmp sgt i32 %19, 16384
  br i1 %20, label %b6, label %b7

b4:
  %21 = load i16, ptr %5, !tbaa !2
  %22 = add nuw i16 %21, 1
  store i16 %22, ptr %5, !tbaa !2
  br label %b2

b5:
  ret i16 40

b6:
  %23 = load i16, ptr %5, !tbaa !2
  ret i16 %23

b7:
  br label %b8

b8:
  %24 = load i32, ptr %7, !tbaa !2
  %25 = call i32 @llvm.smul.fix.i32(i32 8192, i32 %24, i32 12)
  %26 = load i32, ptr %6, !tbaa !2
  %27 = call i32 @llvm.smul.fix.i32(i32 %25, i32 %26, i32 12)
  %28 = add i32 %27, %1
  store i32 %28, ptr %6, !tbaa !2
  %29 = load i32, ptr %3, !tbaa !2
  %30 = load i32, ptr %2, !tbaa !2
  %31 = sub i32 %29, %30
  %32 = add i32 %31, %0
  store i32 %32, ptr %7, !tbaa !2
  br label %b4
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i8
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i32
  %5 = alloca ptr
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i32
  %9 = alloca i32
  %10 = alloca i32
  %11 = alloca i32
  %12 = alloca ptr
  %13 = getelementptr i8, ptr @$str1, i16 6
  store ptr %13, ptr %12, !tbaa !2
  store i32 -9011, ptr %11, !tbaa !2
  store i32 4915, ptr %10, !tbaa !2
  store i32 204, ptr %9, !tbaa !2
  store i32 446, ptr %8, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 22, ptr %6, !tbaa !2
  br label %b2

b2:
  %14 = load i16, ptr %7, !tbaa !2
  %15 = load i16, ptr %6, !tbaa !2
  %16 = icmp slt i16 %14, %15
  br i1 %16, label %b3, label %b5

b3:
  %17 = getelementptr i8, ptr @$str2, i16 6
  store ptr %17, ptr %5, !tbaa !2
  %18 = load i32, ptr %10, !tbaa !2
  %19 = load i16, ptr %7, !tbaa !2
  %20 = sext i16 %19 to i32
  %21 = shl i32 %20, 12
  %22 = load i32, ptr %8, !tbaa !2
  %23 = call i32 @llvm.smul.fix.i32(i32 %21, i32 %22, i32 12)
  %24 = sub i32 %18, %23
  store i32 %24, ptr %4, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  store i16 64, ptr %2, !tbaa !2
  br label %b6

b4:
  %25 = load i16, ptr %7, !tbaa !2
  %26 = add nsw i16 %25, 1
  store i16 %26, ptr %7, !tbaa !2
  br label %b2

b5:
  %27 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %27)
  %28 = load i32, ptr %9, !tbaa !2
  call cc 1005 addrspace(1) void @N$PQ4(i32 %28, i8 zeroext 12)
  %29 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %29)
  call cc 1005 addrspace(1) void @N$PU2(i16 40)
  call cc 1005 addrspace(1) void @N$PN()
  %30 = load ptr, ptr %12, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %30)
  ret i16 0

b6:
  %31 = load i16, ptr %3, !tbaa !2
  %32 = load i16, ptr %2, !tbaa !2
  %33 = icmp slt i16 %31, %32
  br i1 %33, label %b7, label %b9

b7:
  %34 = load i32, ptr %11, !tbaa !2
  %35 = load i16, ptr %3, !tbaa !2
  %36 = sext i16 %35 to i32
  %37 = shl i32 %36, 12
  %38 = load i32, ptr %9, !tbaa !2
  %39 = call i32 @llvm.smul.fix.i32(i32 %37, i32 %38, i32 12)
  %40 = add i32 %34, %39
  %41 = load i32, ptr %4, !tbaa !2
  %42 = call cc 1005 addrspace(1) i16 @escape(i32 %40, i32 %41)
  store i16 %42, ptr %1, !tbaa !2
  %43 = load ptr, ptr %5, !tbaa !2
  %44 = getelementptr i8, ptr %43, i16 -4
  %45 = load i16, ptr %44
  %46 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %43, i16 1, i16 1)
  store ptr %46, ptr %5, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %46, i16 %45
  %48 = load i16, ptr %1, !tbaa !2
  %49 = icmp eq i16 %48, 40
  br i1 %49, label %b10, label %b11

b8:
  %50 = load i16, ptr %3, !tbaa !2
  %51 = add nsw i16 %50, 1
  store i16 %51, ptr %3, !tbaa !2
  br label %b6

b9:
  %52 = load ptr, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %52)
  call cc 1005 addrspace(1) void @N$PN()
  %53 = load ptr, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %53)
  br label %b4

b10:
  store i8 64, ptr %0, !tbaa !2
  br label %b12

b11:
  %54 = load ptr, ptr %12, !tbaa !2
  %55 = load i16, ptr %1, !tbaa !2
  %56 = urem i16 %55, 9
  %57 = getelementptr i8, ptr %54, i16 -4
  %58 = load i16, ptr %57
  %59 = icmp ult i16 %56, %58
  br i1 %59, label %b13, label %b14

b12:
  %60 = load i8, ptr %0, !tbaa !2
  store i8 %60, ptr %47
  %61 = getelementptr i8, ptr %46, i16 -4
  %62 = load i16, ptr %61
  %63 = getelementptr inbounds i8, ptr %46, i16 %62
  store i8 0, ptr %63
  br label %b8

b13:
  %64 = getelementptr inbounds i8, ptr %54, i16 %56
  %65 = load i8, ptr %64
  store i8 %65, ptr %0, !tbaa !2
  br label %b12

b14:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.sdiv.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind willreturn memory(none)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PQ4(i32, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
