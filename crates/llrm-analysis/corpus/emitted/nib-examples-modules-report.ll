target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [8 x i8] c"\08\00\01\00\01\00,\00"
@$str2 = internal constant [12 x i8] c"\08\00\05\00\05\00 left\00"
@$str3 = internal constant [13 x i8] c"\08\00\06\00\06\00 right\00"

define internal i32 @shapes.geometry.Point.shifted(ptr addrspace(1) %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
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

define internal i8 @shapes.geometry.side(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca i8
  store i8 0, ptr %1
  %2 = load i16, ptr addrspace(1) %0
  %3 = icmp slt i16 %2, 0
  %4 = sext i1 %3 to i8
  %5 = icmp ne i8 %4, 0
  br i1 %5, label %b2, label %b3

b2:
  store i8 0, ptr %1, !tbaa !2
  br label %b4

b3:
  store i8 1, ptr %1, !tbaa !2
  br label %b4

b4:
  %6 = load i8, ptr %1, !tbaa !2
  ret i8 %6
}

define internal i16 @shapes.geometry.secret() addrspace(1) {
b1:
  ret i16 7
}

define internal ptr @describe(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = call addrspace(1) i8 @shapes.geometry.side(ptr addrspace(1) %0)
  %2 = icmp eq i8 %1, 0
  %3 = sext i1 %2 to i8
  %4 = icmp ne i8 %3, 0
  br i1 %4, label %b4, label %b3

b3:
  call addrspace(1) void @N$PBEG()
  %5 = load i16, ptr addrspace(1) %0
  call addrspace(1) void @N$PI2(i16 %5)
  %6 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %6)
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %8 = load i16, ptr addrspace(1) %7
  call addrspace(1) void @N$PI2(i16 %8)
  %9 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %9)
  %10 = call addrspace(1) ptr @N$PEND()
  ret ptr %10

b4:
  call addrspace(1) void @N$PBEG()
  %11 = load i16, ptr addrspace(1) %0
  call addrspace(1) void @N$PI2(i16 %11)
  %12 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %12)
  %13 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %14 = load i16, ptr addrspace(1) %13
  call addrspace(1) void @N$PI2(i16 %14)
  %15 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %15)
  %16 = call addrspace(1) ptr @N$PEND()
  ret ptr %16
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PBEG() addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare ptr @N$PEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
