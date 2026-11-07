target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 void @sort(i16 %0, i16 %1, ptr addrspace(1) noalias nocapture %2, i16 %3, i16 %4) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %5 = icmp sge i16 %3, %4
  br i1 %5, label %common.ret, label %b4

common.ret:
  ret void

b4:
  %6 = icmp ult i16 %4, %0
  br i1 %6, label %b5, label %b6, !prof !20

b5:
  %7 = mul i16 %4, 2
  %8 = getelementptr inbounds i8, ptr addrspace(1) %2, i16 %7
  %9 = load i16, ptr addrspace(1) %8
  br label %b7

b6:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b7:
  %10 = phi i16 [ %3, %b5 ], [ %16, %b13 ]
  %11 = phi i16 [ %3, %b5 ], [ %17, %b13 ]
  %12 = icmp slt i16 %11, %4
  br i1 %12, label %b8, label %b10

b8:
  %13 = icmp ult i16 %11, %0
  br i1 %13, label %b14, label %b15, !prof !20

b10:
  %14 = icmp ult i16 %10, %0
  br i1 %14, label %b24, label %b25, !prof !20

b11:
  %15 = icmp ult i16 %10, %0
  br i1 %15, label %b16, label %b17, !prof !20

b13:
  %16 = phi i16 [ %25, %b22 ], [ %10, %b14 ]
  %17 = add nsw i16 %11, 1
  br label %b7

b14:
  %18 = mul i16 %11, 2
  %19 = getelementptr inbounds i8, ptr addrspace(1) %2, i16 %18
  %20 = load i16, ptr addrspace(1) %19
  %21 = icmp slt i16 %20, %9
  br i1 %21, label %b11, label %b13

b15:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b16:
  %22 = mul i16 %10, 2
  %23 = getelementptr inbounds i8, ptr addrspace(1) %2, i16 %22
  %24 = load i16, ptr addrspace(1) %23
  br i1 true, label %b18, label %b19, !prof !20

b17:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b18:
  br i1 true, label %b20, label %b21, !prof !20

b19:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b20:
  store i16 %20, ptr addrspace(1) %23
  br i1 true, label %b22, label %b23, !prof !20

b21:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b22:
  store i16 %24, ptr addrspace(1) %19
  %25 = add i16 %10, 1
  br label %b13

b23:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b24:
  %26 = mul i16 %10, 2
  %27 = getelementptr inbounds i8, ptr addrspace(1) %2, i16 %26
  %28 = load i16, ptr addrspace(1) %27
  br i1 true, label %b26, label %b27, !prof !20

b25:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b26:
  br i1 true, label %b28, label %b29, !prof !20

b27:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b28:
  %29 = load i16, ptr addrspace(1) %8
  store i16 %29, ptr addrspace(1) %27
  br i1 true, label %b30, label %b31, !prof !20

b29:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b30:
  store i16 %28, ptr addrspace(1) %8
  %30 = sub i16 %10, 1
  call cc 1005 addrspace(1) void @sort(i16 %0, i16 %1, ptr addrspace(1) %2, i16 %3, i16 %30)
  %31 = add i16 %10, 1
  call cc 1005 addrspace(1) void @sort(i16 %0, i16 %1, ptr addrspace(1) %2, i16 %31, i16 %4)
  br label %common.ret

b31:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i32 @bench_quicksort(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [2048 x i8]
  br label %b2

b2:
  %2 = phi i16 [ 0, %b1 ], [ %5, %b3 ]
  %3 = icmp slt i16 %2, 1024
  br i1 %3, label %b3, label %b5

b3:
  %4 = getelementptr inbounds i16, ptr %1, i16 %2
  store i16 0, ptr %4, !tbaa !2
  %5 = add nsw i16 %2, 1
  br label %b2

b5:
  %6 = zext i16 %0 to i32
  br label %b6

b6:
  %7 = phi i32 [ %6, %b5 ], [ %12, %b10 ]
  %8 = phi i16 [ 0, %b5 ], [ %18, %b10 ]
  %9 = icmp slt i16 %8, 1024
  br i1 %9, label %b7, label %b9

b7:
  %10 = mul i32 %7, 25173
  %11 = add i32 %10, 13849
  %12 = and i32 %11, 65535
  %13 = icmp ult i16 %8, 1024
  br i1 %13, label %b10, label %b11, !prof !20

b9:
  %14 = addrspacecast ptr %1 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @sort(i16 1024, i16 1024, ptr addrspace(1) %14, i16 0, i16 1023)
  br label %b12

b10:
  %15 = and i32 %12, 32767
  %16 = trunc i32 %15 to i16
  %17 = getelementptr inbounds i16, ptr %1, i16 %8
  store i16 %16, ptr %17, !tbaa !2
  %18 = add nsw i16 %8, 1
  br label %b6

b11:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b12:
  %19 = phi i16 [ 1, %b9 ], [ %23, %b18 ]
  %20 = icmp slt i16 %19, 1024
  br i1 %20, label %b13, label %b15

b13:
  %21 = sub i16 %19, 1
  %22 = icmp ult i16 %21, 1024
  br i1 %22, label %b19, label %b20, !prof !20

b15:
  br label %b23

common.ret:
  %common.op = phi i32 [ %30, %b26 ], [ -1, %b21 ]
  ret i32 %common.op

b18:
  %23 = add nsw i16 %19, 1
  br label %b12

b19:
  %24 = getelementptr inbounds i16, ptr %1, i16 %21
  %25 = load i16, ptr %24, !tbaa !2
  %26 = icmp ult i16 %19, 1024
  br i1 %26, label %b21, label %b22, !prof !20

b20:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b21:
  %27 = getelementptr inbounds i16, ptr %1, i16 %19
  %28 = load i16, ptr %27, !tbaa !2
  %29 = icmp sgt i16 %25, %28
  br i1 %29, label %common.ret, label %b18

b22:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b23:
  %30 = phi i32 [ 0, %b15 ], [ %41, %b27 ]
  %31 = phi i16 [ 0, %b15 ], [ %42, %b27 ]
  %32 = icmp slt i16 %31, 1024
  br i1 %32, label %b24, label %b26

b24:
  %33 = icmp ult i16 %31, 1024
  br i1 %33, label %b27, label %b28, !prof !20

b26:
  br label %common.ret

b27:
  %34 = getelementptr inbounds i16, ptr %1, i16 %31
  %35 = load i16, ptr %34, !tbaa !2
  %36 = sext i16 %35 to i32
  %37 = and i16 %31, 15
  %38 = add i16 %37, 1
  %39 = sext i16 %38 to i32
  %40 = mul i32 %36, %39
  %41 = add i32 %30, %40
  %42 = add nsw i16 %31, 1
  br label %b23

b28:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = call cc 1005 addrspace(1) i32 @bench_quicksort(i16 1)
  call cc 1005 addrspace(1) void @N$PI4(i32 %0)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !5, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !5, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"referent", !0, i64 0}
!6 = !{!5, !5, i64 0}
!7 = !{!"absolute", !0, i64 0}
!8 = !{!7, !7, i64 0}
!9 = !{i16 0, i16 -32768}
!10 = !{i16 0, i16 -32768}
!11 = !{i16 0, i16 -32768}
!12 = !{i16 0, i16 -32768}
!13 = !{i16 0, i16 -32768}
!14 = !{i16 0, i16 -32768}
!15 = !{i16 0, i16 -32768}
!16 = !{i16 0, i16 -32768}
!17 = !{i16 0, i16 -32768}
!18 = !{i16 0, i16 -32768}
!19 = !{!"branch_weights", i32 1, i32 1048575}
!20 = !{!"branch_weights", i32 1048575, i32 1}
