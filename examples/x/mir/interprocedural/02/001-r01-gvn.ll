@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [11 x i8] c"\08\00\04\00\04\00bolt\00"
@$str3 = internal constant [11 x i8] c"\08\00\04\00\04\00gear\00"
@$str4 = internal constant [10 x i8] c"\08\00\03\00\03\00cog\00"
@$str5 = internal constant [10 x i8] c"\08\00\03\00\03\00pin\00"
@$str6 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str7 = internal constant [11 x i8] c"\08\00\04\00\04\00 at \00"
@$str8 = internal constant [13 x i8] c"\08\00\06\00\06\00no pin\00"
@$str9 = internal constant [21 x i8] c"\08\00\0E\00\0E\00cheapest gear \00"
@$str10 = internal constant [14 x i8] c"\08\00\07\00\07\00no gear\00"
@$str11 = internal constant [24 x i8] c"\08\00\11\00\11\00 under 20, first \00"
@$str12 = internal constant [12 x i8] c"\08\00\05\00\05\00low: \00"
@$str13 = internal constant [9 x i8] c"\08\00\02\00\02\00 (\00"
@$str14 = internal constant [8 x i8] c"\08\00\01\00\01\00)\00"
@$str15 = internal constant [10 x i8] c"\08\00\03\00\03\00nut\00"
@$str16 = internal constant [13 x i8] c"\08\00\06\00\06\00 parts\00"

declare internal void @Catalog.add(ptr addrspace(1) nonnull dereferenceable(2) noalias nocapture, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture, i16, i16) addrspace(1) nearcode

declare internal void @north(ptr addrspace(1) nocapture) addrspace(1) nearcode memory(readwrite, argmem: write)

declare internal void @south(ptr addrspace(1) nocapture) addrspace(1) nearcode memory(readwrite, argmem: write)

declare internal void @find(ptr addrspace(1) nocapture, ptr, ptr, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

declare internal ptr addrspace(1) @cheaper(ptr addrspace(1) nonnull dereferenceable(6) readonly noalias, ptr addrspace(1) nonnull dereferenceable(6) readonly noalias) addrspace(1) nearcode memory(argmem: read) willreturn norecurse

declare internal void @affordable(ptr addrspace(1) nocapture, ptr, i16) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

declare internal void @initial(ptr addrspace(1) nocapture, ptr) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

declare i16 @main() addrspace(1) nearcode memory(readwrite, argmem: none)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

declare void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare void @llvm.memcpy.p0.p0.i16(ptr nocapture writeonly, ptr nocapture readonly, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: readwrite)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare void @N$PV(ptr addrspace(1)) addrspace(1)

define internal void @pipeline.body(ptr addrspace(1) nocapture %0, ptr %1, ptr %2, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %3) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite) {
b1:
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = getelementptr i8, ptr %1, i16 -4
  %7 = load i16, ptr %6
  %8 = getelementptr inbounds i8, ptr %5, i16 2
  %9 = getelementptr inbounds i8, ptr %5, i16 4
  %10 = addrspacecast ptr %5 to ptr addrspace(1)
  br label %b2

b2:
  %11 = phi i16 [ 0, %b1 ], [ %13, %b4 ]
  %12 = icmp ult i16 %11, %7
  br i1 %12, label %b3, label %b5

b3:
  br i1 %12, label %b6, label %b7

b4:
  %13 = add nuw i16 %11, 1
  br label %b2

b5:
  %14 = getelementptr i8, ptr %2, i16 -4
  %15 = load i16, ptr %14
  %16 = getelementptr inbounds i8, ptr %4, i16 2
  %17 = getelementptr inbounds i8, ptr %4, i16 4
  %18 = addrspacecast ptr %4 to ptr addrspace(1)
  br label %b13

b6:
  %19 = mul i16 %11, 6
  %20 = getelementptr inbounds i8, ptr %1, i16 %19
  %21 = load ptr, ptr %20
  %22 = getelementptr i8, ptr %21, i16 -4
  %23 = load i16, ptr %22
  %24 = addrspacecast ptr %21 to ptr addrspace(1)
  store i16 %23, ptr %5, !tbaa !2
  store i16 %23, ptr %8, !tbaa !2
  store ptr addrspace(1) %24, ptr %9, !tbaa !2
  %25 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %10, ptr addrspace(1) %3)
  %26 = icmp eq i8 %25, 0
  br i1 %26, label %b8, label %b4

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %27 = phi i16 [ %11, %b6 ]
  %28 = icmp ult i16 %27, %7
  br i1 %28, label %b11, label %b12

b11:
  %29 = mul i16 %27, 6
  %30 = getelementptr inbounds i8, ptr %1, i16 %29
  %31 = addrspacecast ptr %30 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %32 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %31, ptr addrspace(1) %32
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %33 = phi i16 [ 0, %b5 ], [ %35, %b15 ]
  %34 = icmp ult i16 %33, %15
  br i1 %34, label %b14, label %b16

b14:
  br i1 %34, label %b17, label %b18

b15:
  %35 = add nuw i16 %33, 1
  br label %b13

b16:
  store i8 1, ptr addrspace(1) %0
  ret void

b17:
  %36 = mul i16 %33, 6
  %37 = getelementptr inbounds i8, ptr %2, i16 %36
  %38 = load ptr, ptr %37
  %39 = getelementptr i8, ptr %38, i16 -4
  %40 = load i16, ptr %39
  %41 = addrspacecast ptr %38 to ptr addrspace(1)
  store i16 %40, ptr %4, !tbaa !2
  store i16 %40, ptr %16, !tbaa !2
  store ptr addrspace(1) %41, ptr %17, !tbaa !2
  %42 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %18, ptr addrspace(1) %3)
  %43 = icmp eq i8 %42, 0
  br i1 %43, label %b19, label %b15

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %44 = phi i16 [ %33, %b17 ]
  %45 = icmp ult i16 %44, %15
  br i1 %45, label %b22, label %b23

b22:
  %46 = mul i16 %44, 6
  %47 = getelementptr inbounds i8, ptr %2, i16 %46
  %48 = addrspacecast ptr %47 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %49 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %48, ptr addrspace(1) %49
  ret void

b23:
  call addrspace(1) void @N$EBND()
  unreachable
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i16 0, i16 10923}
!7 = !{i8 0, i8 2}
