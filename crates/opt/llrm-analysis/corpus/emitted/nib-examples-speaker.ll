target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [27 x i8] c"\08\00\14\00\14\00playing; a key stops\00"
@$str2 = internal constant [14 x i8] c"\08\00\07\00\07\00stopped\00"
@$str3 = internal constant [11 x i8] c"\08\00\04\00\04\00done\00"

define internal cc 1005 i32 @now() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = call { i16, i16 } @llrm.ia16.asm.cd1a.ax.cx_dx.ax_flags.n(i16 0)
  %5 = extractvalue { i16, i16 } %4, 0
  %6 = extractvalue { i16, i16 } %4, 1
  store i16 %5, ptr %3, !tbaa !2
  %7 = load i16, ptr %3, !tbaa !2
  store i16 %7, ptr %2, !tbaa !2
  store i16 %6, ptr %1, !tbaa !2
  %8 = load i16, ptr %1, !tbaa !2
  store i16 %8, ptr %0, !tbaa !2
  %9 = load i16, ptr %2, !tbaa !2
  %10 = zext i16 %9 to i32
  %11 = shl i32 %10, 16
  %12 = load i16, ptr %0, !tbaa !2
  %13 = zext i16 %12 to i32
  %14 = or i32 %11, %13
  ret i32 %14
}

define internal cc 1005 void @tone(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = zext i16 %0 to i32
  %3 = udiv i32 1193182, %2
  %4 = trunc i32 %3 to i16
  store i16 %4, ptr %1, !tbaa !2
  %5 = load i16, ptr %1, !tbaa !2
  call void @llrm.ia16.asm.b0b6e64388d8e64288f8e642e4610c03e661.bx.-.ax_flags.n(i16 %5)
  ret void
}

define internal cc 1005 void @quiet() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  call void @llrm.ia16.asm.e46124fce661.-.-.ax_flags.n()
  ret void
}

define internal cc 1005 i8 @key_pressed() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = call i16 @llrm.ia16.asm.b401cd16b800007405cd16b80100.-.ax.flags.n()
  store i16 %2, ptr %1, !tbaa !2
  %3 = load i16, ptr %1, !tbaa !2
  store i16 %3, ptr %0, !tbaa !2
  %4 = load i16, ptr %0, !tbaa !2
  %5 = icmp ne i16 %4, 0
  %6 = zext i1 %5 to i8
  ret i8 %6
}

define internal cc 1005 i8 @wait(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i32
  %2 = alloca i16
  %3 = alloca i32
  %4 = call cc 1005 addrspace(1) i32 @now()
  store i32 %4, ptr %3, !tbaa !2
  store i16 %0, ptr %2, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %2, !tbaa !2
  %6 = icmp ugt i16 %5, 0
  br i1 %6, label %b3, label %b4

b3:
  %7 = call cc 1005 addrspace(1) i8 @key_pressed()
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b5, label %b6

b4:
  ret i8 1

b5:
  ret i8 0

b6:
  br label %b7

b7:
  %9 = call cc 1005 addrspace(1) i32 @now()
  store i32 %9, ptr %1, !tbaa !2
  %10 = load i32, ptr %1, !tbaa !2
  %11 = load i32, ptr %3, !tbaa !2
  %12 = icmp ne i32 %10, %11
  br i1 %12, label %b8, label %b9

b8:
  %13 = load i32, ptr %1, !tbaa !2
  store i32 %13, ptr %3, !tbaa !2
  %14 = load i16, ptr %2, !tbaa !2
  %15 = sub i16 %14, 1
  store i16 %15, ptr %2, !tbaa !2
  br label %b10

b9:
  br label %b10

b10:
  br label %b2
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i8
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca [60 x i8]
  store i16 15, ptr %2, !tbaa !2
  store i16 15, ptr %3, !tbaa !2
  %5 = getelementptr inbounds [4 x i8], ptr %4, i16 0
  store i16 330, ptr %5, !tbaa !2
  %6 = getelementptr inbounds [4 x i8], ptr %4, i16 0
  %7 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 4, ptr %7, !tbaa !2
  %8 = getelementptr inbounds [4 x i8], ptr %4, i16 1
  store i16 330, ptr %8, !tbaa !2
  %9 = getelementptr inbounds [4 x i8], ptr %4, i16 1
  %10 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 4, ptr %10, !tbaa !2
  %11 = getelementptr inbounds [4 x i8], ptr %4, i16 2
  store i16 349, ptr %11, !tbaa !2
  %12 = getelementptr inbounds [4 x i8], ptr %4, i16 2
  %13 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 4, ptr %13, !tbaa !2
  %14 = getelementptr inbounds [4 x i8], ptr %4, i16 3
  store i16 392, ptr %14, !tbaa !2
  %15 = getelementptr inbounds [4 x i8], ptr %4, i16 3
  %16 = getelementptr inbounds i8, ptr %15, i16 2
  store i16 4, ptr %16, !tbaa !2
  %17 = getelementptr inbounds [4 x i8], ptr %4, i16 4
  store i16 392, ptr %17, !tbaa !2
  %18 = getelementptr inbounds [4 x i8], ptr %4, i16 4
  %19 = getelementptr inbounds i8, ptr %18, i16 2
  store i16 4, ptr %19, !tbaa !2
  %20 = getelementptr inbounds [4 x i8], ptr %4, i16 5
  store i16 349, ptr %20, !tbaa !2
  %21 = getelementptr inbounds [4 x i8], ptr %4, i16 5
  %22 = getelementptr inbounds i8, ptr %21, i16 2
  store i16 4, ptr %22, !tbaa !2
  %23 = getelementptr inbounds [4 x i8], ptr %4, i16 6
  store i16 330, ptr %23, !tbaa !2
  %24 = getelementptr inbounds [4 x i8], ptr %4, i16 6
  %25 = getelementptr inbounds i8, ptr %24, i16 2
  store i16 4, ptr %25, !tbaa !2
  %26 = getelementptr inbounds [4 x i8], ptr %4, i16 7
  store i16 294, ptr %26, !tbaa !2
  %27 = getelementptr inbounds [4 x i8], ptr %4, i16 7
  %28 = getelementptr inbounds i8, ptr %27, i16 2
  store i16 4, ptr %28, !tbaa !2
  %29 = getelementptr inbounds [4 x i8], ptr %4, i16 8
  store i16 262, ptr %29, !tbaa !2
  %30 = getelementptr inbounds [4 x i8], ptr %4, i16 8
  %31 = getelementptr inbounds i8, ptr %30, i16 2
  store i16 4, ptr %31, !tbaa !2
  %32 = getelementptr inbounds [4 x i8], ptr %4, i16 9
  store i16 262, ptr %32, !tbaa !2
  %33 = getelementptr inbounds [4 x i8], ptr %4, i16 9
  %34 = getelementptr inbounds i8, ptr %33, i16 2
  store i16 4, ptr %34, !tbaa !2
  %35 = getelementptr inbounds [4 x i8], ptr %4, i16 10
  store i16 294, ptr %35, !tbaa !2
  %36 = getelementptr inbounds [4 x i8], ptr %4, i16 10
  %37 = getelementptr inbounds i8, ptr %36, i16 2
  store i16 4, ptr %37, !tbaa !2
  %38 = getelementptr inbounds [4 x i8], ptr %4, i16 11
  store i16 330, ptr %38, !tbaa !2
  %39 = getelementptr inbounds [4 x i8], ptr %4, i16 11
  %40 = getelementptr inbounds i8, ptr %39, i16 2
  store i16 4, ptr %40, !tbaa !2
  %41 = getelementptr inbounds [4 x i8], ptr %4, i16 12
  store i16 330, ptr %41, !tbaa !2
  %42 = getelementptr inbounds [4 x i8], ptr %4, i16 12
  %43 = getelementptr inbounds i8, ptr %42, i16 2
  store i16 6, ptr %43, !tbaa !2
  %44 = getelementptr inbounds [4 x i8], ptr %4, i16 13
  store i16 294, ptr %44, !tbaa !2
  %45 = getelementptr inbounds [4 x i8], ptr %4, i16 13
  %46 = getelementptr inbounds i8, ptr %45, i16 2
  store i16 2, ptr %46, !tbaa !2
  %47 = getelementptr inbounds [4 x i8], ptr %4, i16 14
  store i16 294, ptr %47, !tbaa !2
  %48 = getelementptr inbounds [4 x i8], ptr %4, i16 14
  %49 = getelementptr inbounds i8, ptr %48, i16 2
  store i16 8, ptr %49, !tbaa !2
  %50 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %50)
  call cc 1005 addrspace(1) void @N$PN()
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %51 = load i16, ptr %1, !tbaa !2
  %52 = icmp ult i16 %51, 15
  br i1 %52, label %b3, label %b5

b3:
  %53 = sub i16 %51, 0
  %54 = getelementptr inbounds [4 x i8], ptr %4, i16 %53
  %55 = load i16, ptr %54, !tbaa !2
  call cc 1005 addrspace(1) void @tone(i16 %55)
  %56 = sub i16 %51, 0
  %57 = getelementptr inbounds [4 x i8], ptr %4, i16 %56
  %58 = getelementptr inbounds i8, ptr %57, i16 2
  %59 = load i16, ptr %58, !tbaa !2
  %60 = call cc 1005 addrspace(1) i8 @wait(i16 %59)
  store i8 %60, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @quiet()
  %61 = load i8, ptr %0, !tbaa !2, !range !5
  %62 = icmp ne i8 %61, 0
  br i1 %62, label %b7, label %b6

b4:
  %63 = load i16, ptr %1, !tbaa !2
  %64 = add i16 %63, 1
  store i16 %64, ptr %1, !tbaa !2
  br label %b2

b5:
  %65 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %65)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0

b6:
  %66 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %66)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 1

b7:
  br label %b8

b8:
  %67 = call cc 1005 addrspace(1) i8 @wait(i16 1)
  br label %b4
}

declare { i16, i16 } @llrm.ia16.asm.cd1a.ax.cx_dx.ax_flags.n(i16) nounwind memory(inaccessiblemem: readwrite)

declare void @llrm.ia16.asm.b0b6e64388d8e64288f8e642e4610c03e661.bx.-.ax_flags.n(i16) nounwind memory(inaccessiblemem: readwrite)

declare void @llrm.ia16.asm.e46124fce661.-.-.ax_flags.n() nounwind memory(inaccessiblemem: readwrite)

declare i16 @llrm.ia16.asm.b401cd16b800007405cd16b80100.-.ax.flags.n() nounwind memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
