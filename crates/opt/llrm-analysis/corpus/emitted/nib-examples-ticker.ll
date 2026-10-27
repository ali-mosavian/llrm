target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$var_ticks = internal global [2 x i8] zeroinitializer, align 2
@$var_previous = internal global [4 x i8] zeroinitializer, align 4
@$address__tick = internal constant ptr addrspace(1) @_tick
@$str4 = internal constant [12 x i8] c"\08\00\05\00\05\00tick \00"
@$str5 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str6 = internal constant [13 x i8] c"\08\00\06\00\06\00 spins\00"
@$str7 = internal constant [13 x i8] c"\08\00\06\00\06\00 ticks\00"

define internal cc 1005 void @std.os.write(ptr addrspace(1) %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) %0, i16 %1)
  ret void
}

define internal cc 1005 ptr addrspace(1) @std.os.text_screen() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca ptr addrspace(1)
  %1 = inttoptr i32 -1207959552 to ptr addrspace(1)
  store ptr addrspace(1) %1, ptr %0, !tbaa !2
  %2 = load ptr addrspace(1), ptr %0, !tbaa !2
  ret ptr addrspace(1) %2
}

define internal cc 1005 ptr addrspace(1) @std.dos.vector(i8 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = call addrspace(1) ptr addrspace(1) @LL$VECTOR(i8 zeroext %0)
  ret ptr addrspace(1) %1
}

define internal cc 1005 void @std.dos.set_vector(i8 %0, ptr addrspace(1) %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  call addrspace(1) void @LL$SET_VECTOR(i8 zeroext %0, ptr addrspace(1) %1)
  ret void
}

define internal cc 1005 void @std.dos.chain(ptr addrspace(1) %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  call addrspace(1) void @LL$CHAIN(ptr addrspace(1) %0)
  ret void
}

define x86_intrcc void @_tick(ptr %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = load volatile i16, ptr @$var_ticks, !tbaa !2
  %2 = add i16 %1, 1
  store volatile i16 %2, ptr @$var_ticks, !tbaa !2
  %3 = load volatile ptr addrspace(1), ptr @$var_previous, !tbaa !2
  call cc 1005 addrspace(1) void @std.dos.chain(ptr addrspace(1) %3)
  ret void
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i32
  %2 = alloca i16
  %3 = call cc 1005 addrspace(1) ptr addrspace(1) @std.dos.vector(i8 zeroext 28)
  store volatile ptr addrspace(1) %3, ptr @$var_previous, !tbaa !2
  %4 = load ptr addrspace(1), ptr @$address__tick, !tbaa !2
  call cc 1005 addrspace(1) void @std.dos.set_vector(i8 zeroext 28, ptr addrspace(1) %4)
  store i16 0, ptr %2, !tbaa !2
  store i32 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %2, !tbaa !2
  %6 = icmp ult i16 %5, 36
  br i1 %6, label %b3, label %b4

b3:
  %7 = load i32, ptr %1, !tbaa !2
  %8 = add i32 %7, 1
  store i32 %8, ptr %1, !tbaa !2
  %9 = load volatile i16, ptr @$var_ticks, !tbaa !2
  store i16 %9, ptr %0, !tbaa !2
  %10 = load i16, ptr %0, !tbaa !2
  %11 = load i16, ptr %2, !tbaa !2
  %12 = add i16 %11, 9
  %13 = icmp uge i16 %10, %12
  br i1 %13, label %b5, label %b6

b4:
  %14 = load volatile ptr addrspace(1), ptr @$var_previous, !tbaa !2
  call cc 1005 addrspace(1) void @std.dos.set_vector(i8 zeroext 28, ptr addrspace(1) %14)
  %15 = load volatile i16, ptr @$var_ticks, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %15)
  %16 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %16)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0

b5:
  %17 = load i16, ptr %0, !tbaa !2
  store i16 %17, ptr %2, !tbaa !2
  %18 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %18)
  %19 = load i16, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %19)
  %20 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %20)
  %21 = load i32, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU4(i32 %21)
  %22 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %22)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b7

b6:
  br label %b7

b7:
  br label %b2
}

declare i32 @LL$WRITE_FILE(i16, ptr addrspace(1), i16) addrspace(1)

declare ptr addrspace(1) @LL$VECTOR(i8) addrspace(1)

declare void @LL$SET_VECTOR(i8, ptr addrspace(1)) addrspace(1)

declare void @LL$CHAIN(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
