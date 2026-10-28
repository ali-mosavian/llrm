target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$var_ticks = internal global [2 x i8] zeroinitializer, align 2
@$var_previous = internal global [4 x i8] zeroinitializer, align 4
@$str4 = internal constant [12 x i8] c"\08\00\05\00\05\00tick \00"
@$str5 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str6 = internal constant [13 x i8] c"\08\00\06\00\06\00 spins\00"
@$str7 = internal constant [13 x i8] c"\08\00\06\00\06\00 ticks\00"

define x86_intrcc void @_tick(ptr nocapture readnone %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
  %2 = load volatile i16, ptr @$var_ticks, !tbaa !2
  %3 = add i16 %2, 1
  store volatile i16 %3, ptr @$var_ticks, !tbaa !2
  %4 = load volatile ptr addrspace(1), ptr @$var_previous, align 4, !tbaa !2
  tail call addrspace(1) void @LL$CHAIN(ptr addrspace(1) %4)
  ret void
}

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = tail call addrspace(1) ptr addrspace(1) @LL$VECTOR(i8 zeroext 28)
  store volatile ptr addrspace(1) %1, ptr @$var_previous, align 4, !tbaa !2
  tail call addrspace(1) void @LL$SET_VECTOR(i8 zeroext 28, ptr addrspace(1) @_tick)
  br label %b3

b3:
  %2 = phi i32 [ 1, %0 ], [ %12, %b7 ]
  %3 = phi i16 [ 0, %0 ], [ %10, %b7 ]
  %4 = load volatile i16, ptr @$var_ticks, !tbaa !2
  %5 = add nuw nsw i16 %3, 9
  %6 = icmp ult i16 %4, %5
  br i1 %6, label %b7, label %b5

7:
  %8 = load volatile ptr addrspace(1), ptr @$var_previous, align 4, !tbaa !2
  tail call addrspace(1) void @LL$SET_VECTOR(i8 zeroext 28, ptr addrspace(1) %8)
  %9 = load volatile i16, ptr @$var_ticks, !tbaa !2
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %9)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  ret i16 0

b5:
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU2(i16 %4)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  tail call cc 1005 addrspace(1) void @N$PU4(i32 %2)
  tail call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  tail call cc 1005 addrspace(1) void @N$PN()
  br label %b7

b7:
  %10 = phi i16 [ %4, %b5 ], [ %3, %b3 ]
  %11 = icmp ult i16 %10, 36
  %12 = add i32 %2, 1
  br i1 %11, label %b3, label %7
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
