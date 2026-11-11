target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @nothing(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  %4 = load i16, ptr %3, align 2, !tbaa !7
  %5 = load i16, ptr %2, align 2
  ret i16 %5
}

; Function Attrs: nounwind optsize
define dso_local i64 @nothing_wide(i64 noundef %0) #0 {
  %2 = alloca i64, align 2
  %3 = alloca i64, align 2
  store i64 %0, ptr %3, align 2, !tbaa !8
  %4 = load i64, ptr %3, align 2, !tbaa !8
  %5 = load i64, ptr %2, align 2
  ret i64 %5
}

; Function Attrs: nounwind optsize
define dso_local i16 @through(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  %3 = load i16, ptr %2, align 2, !tbaa !7
  %4 = call i16 @nothing(i16 noundef %3) #3
  ret i16 %4
}

; Function Attrs: nounwind optsize
define dso_local i64 @through_wide(i64 noundef %0) #0 {
  %2 = alloca i64, align 2
  store i64 %0, ptr %2, align 2, !tbaa !8
  %3 = load i64, ptr %2, align 2, !tbaa !8
  %4 = call i64 @nothing_wide(i64 noundef %3) #3
  ret i64 %4
}

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #4
  %3 = call i16 @nothing(i16 noundef 3) #3
  store i16 %3, ptr %2, align 2, !tbaa !7
  %4 = load i16, ptr %2, align 2, !tbaa !7
  %5 = call i16 @through(i16 noundef 4) #3
  %6 = call i64 @through_wide(i64 noundef 5) #3
  call void @report(i32 noundef 42) #3
  call void @llvm.lifetime.end.p0(ptr %2) #4
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
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"long long", !5, i64 0}
