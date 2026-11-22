; ModuleID = 'byvalue_struct_address.c'
source_filename = "byvalue_struct_address.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8-S8"

%struct.baz = type { i16, i16, i16, i16, i16, i16, i16, i16 }

@sum = internal global i16 0, align 2
@__const.main.x = private unnamed_addr constant %struct.baz { i16 1, i16 2, i16 3, i16 4, i16 5, i16 6, i16 7, i16 8 }, align 2

; Function Attrs: nounwind optsize
define dso_local void @bar(ptr %p, i16 %a0, i16 %a1, i16 %a2, i16 %a3, i16 %a4, i16 %a5, i16 %a6, i16 %a7) #0 {
  %g0 = getelementptr inbounds nuw %struct.baz, ptr %p, i32 0, i32 0
  %v0 = load i16, ptr %g0, align 2
  %m0 = mul nsw i16 %v0, 1
  %g1 = getelementptr inbounds nuw %struct.baz, ptr %p, i32 0, i32 1
  %v1 = load i16, ptr %g1, align 2
  %m1 = mul nsw i16 %v1, 10
  %s1 = add nsw i16 %m0, %m1
  %g2 = getelementptr inbounds nuw %struct.baz, ptr %p, i32 0, i32 2
  %v2 = load i16, ptr %g2, align 2
  %m2 = mul nsw i16 %v2, 100
  %s2 = add nsw i16 %s1, %m2
  %g3 = getelementptr inbounds nuw %struct.baz, ptr %p, i32 0, i32 3
  %v3 = load i16, ptr %g3, align 2
  %m3 = mul nsw i16 %v3, 1000
  %s3 = add nsw i16 %s2, %m3
  %g4 = getelementptr inbounds nuw %struct.baz, ptr %p, i32 0, i32 4
  %v4 = load i16, ptr %g4, align 2
  %m4 = mul nsw i16 %v4, 10000
  %s4 = add nsw i16 %s3, %m4
  %g5 = getelementptr inbounds nuw %struct.baz, ptr %p, i32 0, i32 5
  %v5 = load i16, ptr %g5, align 2
  %m5 = mul nsw i16 %v5, 7
  %s5 = add nsw i16 %s4, %m5
  %g6 = getelementptr inbounds nuw %struct.baz, ptr %p, i32 0, i32 6
  %v6 = load i16, ptr %g6, align 2
  %m6 = mul nsw i16 %v6, 9
  %s6 = add nsw i16 %s5, %m6
  %g7 = getelementptr inbounds nuw %struct.baz, ptr %p, i32 0, i32 7
  %v7 = load i16, ptr %g7, align 2
  %m7 = mul nsw i16 %v7, 11
  %s7 = add nsw i16 %s6, %m7
  %e0 = add nsw i16 %s7, %a0
  %e1 = add nsw i16 %e0, %a1
  %e2 = add nsw i16 %e1, %a2
  %e3 = add nsw i16 %e2, %a3
  %e4 = add nsw i16 %e3, %a4
  %e5 = add nsw i16 %e4, %a5
  %e6 = add nsw i16 %e5, %a6
  %e7 = add nsw i16 %e6, %a7
  store i16 %e7, ptr @sum, align 2
  ret void
}

define dso_local void @foo(ptr noundef %0, ptr noundef byval(%struct.baz) align 2 %1, ptr noundef %2) #0 {
  call void @bar(ptr noundef %1, i16 noundef 6, i16 noundef 7, i16 noundef 8, i16 noundef 9, i16 noundef 10, i16 noundef 11, i16 noundef 12, i16 noundef 13) #3
  ret void
}

define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca %struct.baz, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.memcpy.p0.p0.i16(ptr align 2 %2, ptr align 2 @__const.main.x, i16 16, i1 false)
  call void @foo(ptr noundef null, ptr noundef byval(%struct.baz) align 2 %2, ptr noundef null) #3
  %t3 = load i16, ptr @sum, align 2, !tbaa !10
  %t4 = sext i16 %t3 to i32
  call void @report(i32 noundef %t4) #3
  call void @foo(ptr noundef null, ptr noundef byval(%struct.baz) align 2 %2, ptr noundef null) #3
  %3 = load i16, ptr @sum, align 2, !tbaa !10
  %4 = sext i16 %3 to i32
  call void @report(i32 noundef %4) #3
  call void @llvm.lifetime.end.p0(ptr %2) #4
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i16(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i16, i1 immarg) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { optsize }
attributes #4 = { nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!8, !8, i64 0}
!8 = !{!"p1 _ZTS3baz", !9, i64 0}
!9 = !{!"any pointer", !5, i64 0}
!10 = !{!4, !4, i64 0}
!11 = !{!12, !4, i64 0}
!12 = !{!"baz", !4, i64 0, !4, i64 2, !4, i64 4, !4, i64 6, !4, i64 8}
!13 = !{!12, !4, i64 2}
!14 = !{!12, !4, i64 4}
!15 = !{!12, !4, i64 6}
!16 = !{!12, !4, i64 8}
!17 = !{!18, !18, i64 0}
!18 = !{!"p1 omnipotent char", !9, i64 0}
