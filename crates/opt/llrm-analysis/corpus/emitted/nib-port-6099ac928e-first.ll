target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [12 x i8] c"\08\00\05\00\05\00metal\00"
@$str2 = internal constant [9 x i8] c"\08\00\02\00\02\00ok\00"

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
  %0 = getelementptr i8, ptr @$str1, i16 6
  %1 = call cc 1005 addrspace(1) i8 @first(ptr %0)
  %2 = icmp eq i8 %1, 109
  br i1 %2, label %b2, label %b3

b2:
  %3 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %3)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0

b3:
  br label %b4

b4:
  ret i16 1
}

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
