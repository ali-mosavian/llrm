target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal i16 @mix(i16 %0, i16 %1, i8 %2) addrspace(1) memory(none) willreturn {
b1:
  ret i16 -16387
}

define internal i32 @shifts(i32 %0, i16 %1) addrspace(1) {
b1:
  %2 = icmp ult i16 %1, 32
  br i1 %2, label %b2, label %b3

b2:
  %3 = zext i16 %1 to i32
  %4 = ashr i32 %0, %3
  %5 = shl i32 %0, %3
  %6 = add i32 %4, %5
  ret i32 %6

b3:
  call addrspace(1) void @N$ESHF()
  unreachable
}

define internal i8 @logic(i16 %0, i16 %1) addrspace(1) memory(none) willreturn {
b1:
  %2 = icmp slt i16 %0, %1
  %3 = sext i1 %2 to i8
  br i1 %2, label %b2, label %b3

b2:
  %4 = icmp eq i16 %0, 0
  %5 = sext i1 %4 to i8
  %6 = xor i8 %5, -1
  br label %b3

b3:
  %7 = phi i8 [ %3, %b1 ], [ %6, %b2 ]
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b5, label %b4

b4:
  %9 = icmp eq i16 %1, 7
  %10 = sext i1 %9 to i8
  br label %b5

b5:
  %11 = phi i8 [ %7, %b3 ], [ %10, %b4 ]
  ret i8 %11
}

define internal i32 @widen(i8 %0, i8 %1, i8 %2) addrspace(1) memory(none) willreturn {
b1:
  ret i32 447
}

define internal i16 @narrow(i32 %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = trunc i32 %0 to i8
  %2 = sext i8 %1 to i16
  %3 = zext i8 %1 to i16
  %4 = add i16 %2, %3
  ret i16 %4
}

define internal i16 @filled(i16 %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = alloca [18 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 18, i1 false)
  br label %b2

b2:
  %2 = phi i16 [ 0, %b1 ], [ %5, %b3 ]
  %3 = icmp slt i16 %2, 9
  br i1 %3, label %b3, label %b5

b3:
  %4 = getelementptr inbounds i16, ptr %1, i16 %2
  store i16 15, ptr %4, !tbaa !2
  %5 = add i16 %2, 1
  br label %b2

b5:
  %6 = getelementptr inbounds i16, ptr %1, i16 4
  store i16 1, ptr %6, !tbaa !2
  br label %b6

b6:
  %7 = phi i16 [ 0, %b5 ], [ %12, %b7 ]
  %8 = phi i16 [ 0, %b5 ], [ %13, %b7 ]
  %9 = icmp ult i16 %8, 9
  br i1 %9, label %b7, label %b9

b7:
  %10 = getelementptr inbounds i16, ptr %1, i16 %8
  %11 = load i16, ptr %10, !tbaa !2
  %12 = add i16 %7, %11
  %13 = add i16 %8, 1
  br label %b6

b9:
  ret i16 %7
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [18 x i8]
  call addrspace(1) void @N$PU2(i16 -16387)
  %1 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %1)
  call addrspace(1) void @N$PI4(i32 -812500)
  call addrspace(1) void @N$PS(ptr %1)
  call addrspace(1) void @N$PI4(i32 3203125)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$PI2(i16 1)
  call addrspace(1) void @N$PS(ptr %1)
  call addrspace(1) void @N$PI2(i16 0)
  call addrspace(1) void @N$PS(ptr %1)
  call addrspace(1) void @N$PI2(i16 1)
  call addrspace(1) void @N$PS(ptr %1)
  call addrspace(1) void @N$PI2(i16 0)
  call addrspace(1) void @N$PN()
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 18, i1 false)
  br label %2

2:
  %3 = phi i16 [ 0, %b1 ], [ %7, %5 ]
  %4 = icmp slt i16 %3, 9
  br i1 %4, label %5, label %8

5:
  %6 = getelementptr inbounds i16, ptr %0, i16 %3
  store i16 15, ptr %6
  %7 = add i16 %3, 1
  br label %2

8:
  %9 = getelementptr inbounds i16, ptr %0, i16 4
  store i16 1, ptr %9
  br label %10

10:
  %11 = phi i16 [ 0, %8 ], [ %17, %14 ]
  %12 = phi i16 [ 0, %8 ], [ %18, %14 ]
  %13 = icmp ult i16 %12, 9
  br i1 %13, label %14, label %19

14:
  %15 = getelementptr inbounds i16, ptr %0, i16 %12
  %16 = load i16, ptr %15
  %17 = add i16 %11, %16
  %18 = add i16 %12, 1
  br label %10

19:
  call addrspace(1) void @N$PI4(i32 447)
  call addrspace(1) void @N$PS(ptr %1)
  call addrspace(1) void @N$PI2(i16 254)
  call addrspace(1) void @N$PS(ptr %1)
  call addrspace(1) void @N$PI2(i16 224)
  call addrspace(1) void @N$PS(ptr %1)
  call addrspace(1) void @N$PI2(i16 %11)
  call addrspace(1) void @N$PN()
  ret i16 0
}

declare void @N$ESHF() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI4(i32) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PI2(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
