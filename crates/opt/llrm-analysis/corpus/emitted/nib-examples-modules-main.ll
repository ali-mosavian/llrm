target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [8 x i8] c"\08\00\01\00\01\00,\00"
@$str2 = internal constant [12 x i8] c"\08\00\05\00\05\00 left\00"
@$str3 = internal constant [13 x i8] c"\08\00\06\00\06\00 right\00"

define internal cc 1005 i32 @shapes.geometry.Point.shifted(ptr addrspace(1) nonnull dereferenceable(4) readonly noalias nocapture %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca [4 x i8]
  %3 = load i16, ptr addrspace(1) %0
  %4 = add i16 %3, %1
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %6 = load i16, ptr addrspace(1) %5
  store i16 %4, ptr %2, !tbaa !2
  %7 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %6, ptr %7, !tbaa !2
  %8 = addrspacecast ptr %2 to ptr addrspace(1)
  %9 = load i32, ptr addrspace(1) %8, !tbaa !2
  ret i32 %9
}

define internal cc 1005 i8 @shapes.geometry.side(ptr addrspace(1) nonnull dereferenceable(4) readonly noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i8
  %2 = load i16, ptr addrspace(1) %0
  %3 = icmp slt i16 %2, 0
  br i1 %3, label %b2, label %b3

b2:
  store i8 0, ptr %1, !tbaa !2
  br label %b4

b3:
  store i8 1, ptr %1, !tbaa !2
  br label %b4

b4:
  %4 = load i8, ptr %1, !tbaa !2
  ret i8 %4
}

define internal cc 1005 i16 @shapes.geometry.secret() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  ret i16 7
}

define internal cc 1005 ptr @report.describe(ptr addrspace(1) nonnull dereferenceable(4) readonly noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = call cc 1005 addrspace(1) i8 @shapes.geometry.side(ptr addrspace(1) %0)
  %2 = icmp eq i8 %1, 0
  br i1 %2, label %b4, label %b3

b3:
  call cc 1005 addrspace(1) void @N$PBEG()
  %3 = load i16, ptr addrspace(1) %0
  call cc 1005 addrspace(1) void @N$PI2(i16 %3)
  %4 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %4)
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %6 = load i16, ptr addrspace(1) %5
  call cc 1005 addrspace(1) void @N$PI2(i16 %6)
  %7 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %7)
  %8 = call cc 1005 addrspace(1) ptr @N$PEND()
  ret ptr %8

b4:
  call cc 1005 addrspace(1) void @N$PBEG()
  %9 = load i16, ptr addrspace(1) %0
  call cc 1005 addrspace(1) void @N$PI2(i16 %9)
  %10 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %10)
  %11 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %12 = load i16, ptr addrspace(1) %11
  call cc 1005 addrspace(1) void @N$PI2(i16 %12)
  %13 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %13)
  %14 = call cc 1005 addrspace(1) ptr @N$PEND()
  ret ptr %14
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca ptr
  %1 = alloca [4 x i8]
  %2 = alloca [4 x i8]
  %3 = alloca ptr
  %4 = alloca [4 x i8]
  store i16 -3, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 4, ptr %5, !tbaa !2
  %6 = addrspacecast ptr %4 to ptr addrspace(1)
  %7 = call cc 1005 addrspace(1) ptr @report.describe(ptr addrspace(1) %6)
  store ptr %7, ptr %3, !tbaa !2
  %8 = load ptr, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %8)
  call cc 1005 addrspace(1) void @N$PN()
  %9 = addrspacecast ptr %4 to ptr addrspace(1)
  %10 = call cc 1005 addrspace(1) i32 @shapes.geometry.Point.shifted(ptr addrspace(1) %9, i16 5)
  %11 = addrspacecast ptr %1 to ptr addrspace(1)
  store i32 %10, ptr addrspace(1) %11, !tbaa !2
  %12 = load i16, ptr %1, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %1, i16 2
  %14 = load i16, ptr %13, !tbaa !2
  store i16 %12, ptr %2, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %14, ptr %15, !tbaa !2
  %16 = addrspacecast ptr %2 to ptr addrspace(1)
  %17 = call cc 1005 addrspace(1) ptr @report.describe(ptr addrspace(1) %16)
  store ptr %17, ptr %0, !tbaa !2
  %18 = load ptr, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %18)
  call cc 1005 addrspace(1) void @N$PN()
  %19 = load ptr, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %19)
  %20 = load ptr, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %20)
  ret i16 0
}

declare cc 1005 void @N$PBEG() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 ptr @N$PEND() addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
