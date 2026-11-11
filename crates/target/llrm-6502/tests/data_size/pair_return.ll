target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i64 @pair(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !7
  %5 = load i32, ptr %3, align 2, !tbaa !7
  %6 = zext i32 %5 to i64
  %7 = shl i64 %6, 32
  %8 = load i32, ptr %4, align 2, !tbaa !7
  %9 = zext i32 %8 to i64
  %10 = or i64 %7, %9
  ret i64 %10
}

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i64, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  %3 = call i64 @pair(i32 noundef 286331153, i32 noundef 572662306) #4
  store i64 %3, ptr %2, align 2, !tbaa !9
  %4 = load i64, ptr %2, align 2, !tbaa !9
  %5 = lshr i64 %4, 32
  %6 = trunc i64 %5 to i32
  call void @report(i32 noundef %6) #4
  %7 = load i64, ptr %2, align 2, !tbaa !9
  %8 = trunc i64 %7 to i32
  call void @report(i32 noundef %8) #4
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }
attributes #4 = { optsize }

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
!8 = !{!"long", !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"long long", !5, i64 0}
