source_filename = "structs.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

%struct.point = type { i32, i32 }
%struct.big = type { i64, i8, i16, double, %struct.point }

@__const.main.a = private unnamed_addr constant %struct.point { i32 3, i32 4 }, align 4
@__const.main.b = private unnamed_addr constant %struct.point { i32 5, i32 -6 }, align 4
@.str = private unnamed_addr constant [11 x i8] c"%d %d %ld\0A\00", align 1
@.str.1 = private unnamed_addr constant [15 x i8] c"%ld %d %d %ld\0A\00", align 1
@.str.2 = private unnamed_addr constant [7 x i8] c"%d %d\0A\00", align 1

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca %struct.point, align 4
  %3 = alloca %struct.point, align 4
  %4 = alloca %struct.point, align 4
  %5 = alloca %struct.big, align 8
  %6 = alloca %struct.big, align 8
  %7 = alloca %struct.big, align 8
  store i32 0, ptr %1, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %2, ptr align 4 @__const.main.a, i64 8, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %3, ptr align 4 @__const.main.b, i64 8, i1 false)
  %8 = load i64, ptr %2, align 4
  %9 = load i64, ptr %3, align 4
  %10 = call i64 @add(i64 %8, i64 %9)
  store i64 %10, ptr %4, align 4
  %11 = getelementptr inbounds nuw %struct.point, ptr %4, i32 0, i32 0
  %12 = load i32, ptr %11, align 4, !tbaa !9
  %13 = getelementptr inbounds nuw %struct.point, ptr %4, i32 0, i32 1
  %14 = load i32, ptr %13, align 4, !tbaa !11
  %15 = call i64 @dot(ptr noundef %2, ptr noundef %3)
  %16 = call i32 (ptr, ...) @printf(ptr noundef @.str, i32 noundef %12, i32 noundef %14, i64 noundef %15)
  call void @make(ptr dead_on_unwind writable sret(%struct.big) align 8 %5, i64 noundef 1000000000000, i32 noundef 7)
  %17 = getelementptr inbounds nuw %struct.big, ptr %5, i32 0, i32 0
  %18 = load i64, ptr %17, align 8, !tbaa !12
  %19 = getelementptr inbounds nuw %struct.big, ptr %5, i32 0, i32 1
  %20 = load i8, ptr %19, align 8, !tbaa !17
  %21 = sext i8 %20 to i32
  %22 = getelementptr inbounds nuw %struct.big, ptr %5, i32 0, i32 2
  %23 = load i16, ptr %22, align 2, !tbaa !18
  %24 = sext i16 %23 to i32
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %6, ptr align 8 %5, i64 32, i1 false), !tbaa.struct !19
  %25 = call i64 @use(ptr nofreeobj noundef align 8 dead_on_return dereferenceable(32) %6)
  %26 = call i32 (ptr, ...) @printf(ptr noundef @.str.1, i64 noundef %18, i32 noundef %21, i32 noundef %24, i64 noundef %25)
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %7, ptr align 8 %5, i64 32, i1 false), !tbaa.struct !19
  %27 = getelementptr inbounds nuw %struct.big, ptr %7, i32 0, i32 4
  %28 = getelementptr inbounds nuw %struct.point, ptr %27, i32 0, i32 0
  store i32 99, ptr %28, align 8, !tbaa !25
  %29 = getelementptr inbounds nuw %struct.big, ptr %5, i32 0, i32 4
  %30 = getelementptr inbounds nuw %struct.point, ptr %29, i32 0, i32 0
  %31 = load i32, ptr %30, align 8, !tbaa !25
  %32 = getelementptr inbounds nuw %struct.big, ptr %7, i32 0, i32 4
  %33 = getelementptr inbounds nuw %struct.point, ptr %32, i32 0, i32 0
  %34 = load i32, ptr %33, align 8, !tbaa !25
  %35 = call i32 (ptr, ...) @printf(ptr noundef @.str.2, i32 noundef %31, i32 noundef %34)
  ret i32 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i64, i1 immarg) #1

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @add(i64 %0, i64 %1) #0 {
  %3 = alloca %struct.point, align 4
  %4 = alloca %struct.point, align 4
  %5 = alloca %struct.point, align 4
  store i64 %0, ptr %4, align 4
  store i64 %1, ptr %5, align 4
  %6 = getelementptr inbounds nuw %struct.point, ptr %3, i32 0, i32 0
  %7 = getelementptr inbounds nuw %struct.point, ptr %4, i32 0, i32 0
  %8 = load i32, ptr %7, align 4, !tbaa !9
  %9 = getelementptr inbounds nuw %struct.point, ptr %5, i32 0, i32 0
  %10 = load i32, ptr %9, align 4, !tbaa !9
  %11 = add nsw i32 %8, %10
  store i32 %11, ptr %6, align 4, !tbaa !9
  %12 = getelementptr inbounds nuw %struct.point, ptr %3, i32 0, i32 1
  %13 = getelementptr inbounds nuw %struct.point, ptr %4, i32 0, i32 1
  %14 = load i32, ptr %13, align 4, !tbaa !11
  %15 = getelementptr inbounds nuw %struct.point, ptr %5, i32 0, i32 1
  %16 = load i32, ptr %15, align 4, !tbaa !11
  %17 = add nsw i32 %14, %16
  store i32 %17, ptr %12, align 4, !tbaa !11
  %18 = load i64, ptr %3, align 4
  ret i64 %18
}

declare i32 @printf(ptr noundef, ...) #2

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @dot(ptr noundef %0, ptr noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca ptr, align 8
  store ptr %0, ptr %3, align 8, !tbaa !26
  store ptr %1, ptr %4, align 8, !tbaa !26
  %5 = load ptr, ptr %3, align 8, !tbaa !26
  %6 = getelementptr inbounds nuw %struct.point, ptr %5, i32 0, i32 0
  %7 = load i32, ptr %6, align 4, !tbaa !9
  %8 = sext i32 %7 to i64
  %9 = load ptr, ptr %4, align 8, !tbaa !26
  %10 = getelementptr inbounds nuw %struct.point, ptr %9, i32 0, i32 0
  %11 = load i32, ptr %10, align 4, !tbaa !9
  %12 = sext i32 %11 to i64
  %13 = mul nsw i64 %8, %12
  %14 = load ptr, ptr %3, align 8, !tbaa !26
  %15 = getelementptr inbounds nuw %struct.point, ptr %14, i32 0, i32 1
  %16 = load i32, ptr %15, align 4, !tbaa !11
  %17 = sext i32 %16 to i64
  %18 = load ptr, ptr %4, align 8, !tbaa !26
  %19 = getelementptr inbounds nuw %struct.point, ptr %18, i32 0, i32 1
  %20 = load i32, ptr %19, align 4, !tbaa !11
  %21 = sext i32 %20 to i64
  %22 = mul nsw i64 %17, %21
  %23 = add nsw i64 %13, %22
  ret i64 %23
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @make(ptr dead_on_unwind noalias writable sret(%struct.big) align 8 %0, i64 noundef %1, i32 noundef %2) #0 {
  %4 = alloca i64, align 8
  %5 = alloca i32, align 4
  store i64 %1, ptr %4, align 8, !tbaa !20
  store i32 %2, ptr %5, align 4, !tbaa !24
  %6 = load i64, ptr %4, align 8, !tbaa !20
  %7 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 0
  store i64 %6, ptr %7, align 8, !tbaa !12
  %8 = load i32, ptr %5, align 4, !tbaa !24
  %9 = trunc i32 %8 to i8
  %10 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 1
  store i8 %9, ptr %10, align 8, !tbaa !17
  %11 = load i32, ptr %5, align 4, !tbaa !24
  %12 = mul nsw i32 %11, 3
  %13 = trunc i32 %12 to i16
  %14 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 2
  store i16 %13, ptr %14, align 2, !tbaa !18
  %15 = load i32, ptr %5, align 4, !tbaa !24
  %16 = sitofp i32 %15 to double
  %17 = fmul double %16, 1.500000e+00
  %18 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 3
  store double %17, ptr %18, align 8, !tbaa !29
  %19 = load i32, ptr %5, align 4, !tbaa !24
  %20 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 4
  %21 = getelementptr inbounds nuw %struct.point, ptr %20, i32 0, i32 0
  store i32 %19, ptr %21, align 8, !tbaa !25
  %22 = load i32, ptr %5, align 4, !tbaa !24
  %23 = sub nsw i32 0, %22
  %24 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 4
  %25 = getelementptr inbounds nuw %struct.point, ptr %24, i32 0, i32 1
  store i32 %23, ptr %25, align 4, !tbaa !30
  ret void
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @use(ptr nofreeobj noundef align 8 dead_on_return dereferenceable(32) %0) #0 {
  %2 = alloca ptr, align 8
  store ptr %0, ptr %2, align 8, !tbaa !31
  %3 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 0
  %4 = load i64, ptr %3, align 8, !tbaa !12
  %5 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 1
  %6 = load i8, ptr %5, align 8, !tbaa !17
  %7 = sext i8 %6 to i64
  %8 = add nsw i64 %4, %7
  %9 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 2
  %10 = load i16, ptr %9, align 2, !tbaa !18
  %11 = sext i16 %10 to i64
  %12 = add nsw i64 %8, %11
  %13 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 3
  %14 = load double, ptr %13, align 8, !tbaa !29
  %15 = fptosi double %14 to i64
  %16 = add nsw i64 %12, %15
  %17 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 4
  %18 = getelementptr inbounds nuw %struct.point, ptr %17, i32 0, i32 0
  %19 = load i32, ptr %18, align 8, !tbaa !25
  %20 = sext i32 %19 to i64
  %21 = add nsw i64 %16, %20
  %22 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 4
  %23 = getelementptr inbounds nuw %struct.point, ptr %22, i32 0, i32 1
  %24 = load i32, ptr %23, align 4, !tbaa !30
  %25 = sext i32 %24 to i64
  %26 = add nsw i64 %21, %25
  ret i64 %26
}

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}
!llvm.errno.tbaa = !{!4}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"uwtable", i32 1}
!2 = !{i32 7, !"frame-pointer", i32 4}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = !{!5, !6, i64 0}
!5 = !{!"__libc_errno", !6, i64 0}
!6 = !{!"int", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = !{!10, !6, i64 0}
!10 = !{!"point", !6, i64 0, !6, i64 4}
!11 = !{!10, !6, i64 4}
!12 = !{!13, !14, i64 0}
!13 = !{!"big", !14, i64 0, !7, i64 8, !15, i64 10, !16, i64 16, !10, i64 24}
!14 = !{!"long", !7, i64 0}
!15 = !{!"short", !7, i64 0}
!16 = !{!"double", !7, i64 0}
!17 = !{!13, !7, i64 8}
!18 = !{!13, !15, i64 10}
!19 = !{i64 0, i64 8, !20, i64 8, i64 1, !21, i64 10, i64 2, !22, i64 16, i64 8, !23, i64 24, i64 4, !24, i64 28, i64 4, !24}
!20 = !{!14, !14, i64 0}
!21 = !{!7, !7, i64 0}
!22 = !{!15, !15, i64 0}
!23 = !{!16, !16, i64 0}
!24 = !{!6, !6, i64 0}
!25 = !{!13, !6, i64 24}
!26 = !{!27, !27, i64 0}
!27 = !{!"p1 _ZTS5point", !28, i64 0}
!28 = !{!"any pointer", !7, i64 0}
!29 = !{!13, !16, i64 16}
!30 = !{!13, !6, i64 28}
!31 = !{!32, !32, i64 0}
!32 = !{!"p1 _ZTS3big", !28, i64 0}
