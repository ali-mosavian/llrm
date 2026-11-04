; ModuleID = 'tests/run/c/byvalue_struct_address.c'
source_filename = "tests/run/c/byvalue_struct_address.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

%struct.baz = type { i32, i32, i32, i32, i32 }

@sum = internal global i32 0, align 4
@__const.main.x = private unnamed_addr constant %struct.baz { i32 1, i32 2, i32 3, i32 4, i32 5 }, align 4

; Function Attrs: nounwind ssp
define void @bar(ptr noundef %0, i32 noundef %1, i32 noundef %2, i32 noundef %3, i32 noundef %4, i32 noundef %5) #0 {
  %7 = alloca ptr, align 8
  %8 = alloca i32, align 4
  %9 = alloca i32, align 4
  %10 = alloca i32, align 4
  %11 = alloca i32, align 4
  %12 = alloca i32, align 4
  store ptr %0, ptr %7, align 8, !tbaa !9
  store i32 %1, ptr %8, align 4, !tbaa !12
  store i32 %2, ptr %9, align 4, !tbaa !12
  store i32 %3, ptr %10, align 4, !tbaa !12
  store i32 %4, ptr %11, align 4, !tbaa !12
  store i32 %5, ptr %12, align 4, !tbaa !12
  %13 = load ptr, ptr %7, align 8, !tbaa !9
  %14 = getelementptr inbounds nuw %struct.baz, ptr %13, i32 0, i32 0
  %15 = load i32, ptr %14, align 4, !tbaa !13
  %16 = mul nsw i32 %15, 1
  %17 = load ptr, ptr %7, align 8, !tbaa !9
  %18 = getelementptr inbounds nuw %struct.baz, ptr %17, i32 0, i32 1
  %19 = load i32, ptr %18, align 4, !tbaa !15
  %20 = mul nsw i32 %19, 10
  %21 = add nsw i32 %16, %20
  %22 = load ptr, ptr %7, align 8, !tbaa !9
  %23 = getelementptr inbounds nuw %struct.baz, ptr %22, i32 0, i32 2
  %24 = load i32, ptr %23, align 4, !tbaa !16
  %25 = mul nsw i32 %24, 100
  %26 = add nsw i32 %21, %25
  %27 = load ptr, ptr %7, align 8, !tbaa !9
  %28 = getelementptr inbounds nuw %struct.baz, ptr %27, i32 0, i32 3
  %29 = load i32, ptr %28, align 4, !tbaa !17
  %30 = mul nsw i32 %29, 1000
  %31 = add nsw i32 %26, %30
  %32 = load ptr, ptr %7, align 8, !tbaa !9
  %33 = getelementptr inbounds nuw %struct.baz, ptr %32, i32 0, i32 4
  %34 = load i32, ptr %33, align 4, !tbaa !18
  %35 = mul nsw i32 %34, 10000
  %36 = add nsw i32 %31, %35
  %37 = load i32, ptr %8, align 4, !tbaa !12
  %38 = add nsw i32 %36, %37
  %39 = load i32, ptr %9, align 4, !tbaa !12
  %40 = add nsw i32 %38, %39
  %41 = load i32, ptr %10, align 4, !tbaa !12
  %42 = add nsw i32 %40, %41
  %43 = load i32, ptr %11, align 4, !tbaa !12
  %44 = add nsw i32 %42, %43
  %45 = load i32, ptr %12, align 4, !tbaa !12
  %46 = add nsw i32 %44, %45
  store i32 %46, ptr @sum, align 4, !tbaa !12
  ret void
}

; Function Attrs: nounwind ssp
define void @foo(ptr noundef %0, ptr nofreeobj noundef align 4 dead_on_return dereferenceable(20) %1, ptr noundef %2) #0 {
  %4 = alloca ptr, align 8
  %5 = alloca ptr, align 8
  %6 = alloca ptr, align 8
  store ptr %0, ptr %4, align 8, !tbaa !19
  store ptr %1, ptr %5, align 8, !tbaa !9
  store ptr %2, ptr %6, align 8, !tbaa !19
  call void @bar(ptr noundef %1, i32 noundef 6, i32 noundef 7, i32 noundef 8, i32 noundef 9, i32 noundef 10) #3
  ret void
}

; Function Attrs: nounwind ssp
define i32 @main() #0 {
  %1 = alloca %struct.baz, align 4
  %2 = alloca %struct.baz, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %1, ptr align 4 @__const.main.x, i64 20, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %2, ptr align 4 %1, i64 20, i1 false), !tbaa.struct !21
  call void @foo(ptr noundef null, ptr nofreeobj noundef align 4 dead_on_return dereferenceable(20) %2, ptr noundef null) #3
  %3 = load i32, ptr @sum, align 4, !tbaa !12
  %4 = sext i32 %3 to i64
  call void @report(i64 noundef %4) #3
  ret i32 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i64, i1 immarg) #1

declare void @report(i64 noundef) #2

attributes #0 = { nounwind ssp "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #3 = { nobuiltin "no-builtins" }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}
!llvm.errno.tbaa = !{!4}

!0 = !{i32 2, !"SDK Version", [2 x i32] [i32 27, i32 0]}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"frame-pointer", i32 4}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = !{!5, !6, i64 0}
!5 = !{!"__libc_errno", !6, i64 0}
!6 = !{!"int", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = !{!10, !10, i64 0}
!10 = !{!"p1 _ZTS3baz", !11, i64 0}
!11 = !{!"any pointer", !7, i64 0}
!12 = !{!6, !6, i64 0}
!13 = !{!14, !6, i64 0}
!14 = !{!"baz", !6, i64 0, !6, i64 4, !6, i64 8, !6, i64 12, !6, i64 16}
!15 = !{!14, !6, i64 4}
!16 = !{!14, !6, i64 8}
!17 = !{!14, !6, i64 12}
!18 = !{!14, !6, i64 16}
!19 = !{!20, !20, i64 0}
!20 = !{!"p1 omnipotent char", !11, i64 0}
!21 = !{i64 0, i64 4, !12, i64 4, i64 4, !12, i64 8, i64 4, !12, i64 12, i64 4, !12, i64 16, i64 4, !12}
