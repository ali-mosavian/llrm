target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32"

define internal i32 @bench_sieve() addrspace(1) memory(none) nounwind norecurse {
b1:
  %0 = alloca [1024 x i8]
  %1 = getelementptr inbounds i8, ptr %0, i16 0
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 1024, i1 false)
  br label %b7

b7:
  %2 = phi i16 [ 0, %b1 ], [ %9, %b8 ]
  %3 = phi i32 [ 0, %b1 ], [ %8, %b8 ]
  %4 = phi i16 [ 2, %b1 ], [ %10, %b8 ]
  %5 = getelementptr inbounds i8, ptr %0, i16 %4
  %6 = load i8, ptr %5, !tbaa !2
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b8, label %b14

b8:
  %8 = phi i32 [ %3, %b7 ], [ %19, %b14 ], [ %19, %b20 ]
  %9 = phi i16 [ %2, %b7 ], [ %17, %b14 ], [ %17, %b20 ]
  %10 = add nuw i16 %4, 1
  %11 = icmp ult i16 %10, 1024
  br i1 %11, label %b7, label %b9

b9:
  %12 = phi i16 [ %9, %b8 ]
  %13 = phi i32 [ %8, %b8 ]
  %14 = zext i16 %12 to i32
  %15 = shl i32 %14, 16
  %16 = xor i32 %15, %13
  ret i32 %16

b14:
  %17 = add i16 %2, 1
  %18 = zext i16 %4 to i32
  %19 = add i32 %3, %18
  %20 = icmp ule i16 %4, 31
  br i1 %20, label %b15, label %b8

b15:
  %21 = mul i16 %4, %4
  br label %b18

b18:
  %22 = phi i16 [ %21, %b15 ], [ %25, %b19 ]
  %23 = icmp ult i16 %22, 1024
  br i1 %23, label %b19, label %b20

b19:
  %24 = getelementptr inbounds i8, ptr %0, i16 %22
  store i8 1, ptr %24, !tbaa !2
  %25 = add i16 %22, %4
  br label %b18

b20:
  br label %b8
}

define i16 @main() addrspace(1) memory(inaccessiblemem: readwrite) {
b1:
  %0 = call addrspace(1) i32 @bench_sieve()
  call addrspace(1) void @N$PI4(i32 %0)
  call addrspace(1) void @N$PN()
  ret i16 0
}

declare void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare void @N$PI4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

declare void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
