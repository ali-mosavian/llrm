target triple = "msp430"

; Function Attrs: noinline nounwind optsize
define dso_local i16 @run() #0 {
  %1 = alloca [2 x i16], align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  %2 = getelementptr inbounds [2 x i16], ptr %1, i16 0, i16 0
  store i16 1, ptr %2, align 2, !tbaa !7
  %3 = getelementptr inbounds [2 x i16], ptr %1, i16 0, i16 1
  store i16 2, ptr %3, align 2, !tbaa !7
  %4 = getelementptr inbounds [2 x i16], ptr %1, i16 0, i16 0
  call void @via(ptr noundef %4) #5
  %5 = getelementptr inbounds [2 x i16], ptr %1, i16 0, i16 0
  %6 = load i16, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i16 %6
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal void @via(ptr noundef %0) #2 {
  %2 = alloca ptr, align 2
  store ptr %0, ptr %2, align 2, !tbaa !8
  %3 = load ptr, ptr %2, align 2, !tbaa !8
  call void @set(ptr noundef %3) #5
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define dso_local i16 @main() #2 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i16 @run() #5
  %3 = sext i16 %2 to i32
  call void @report(i32 noundef %3) #5
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #3

; Function Attrs: noinline nounwind optsize
define internal void @set(ptr noundef %0) #0 {
  %2 = alloca ptr, align 2
  store ptr %0, ptr %2, align 2, !tbaa !8
  %3 = load ptr, ptr %2, align 2, !tbaa !8
  %4 = getelementptr inbounds i16, ptr %3, i16 0
  store i16 5, ptr %4, align 2, !tbaa !7
  ret void
}

attributes #0 = { noinline nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #4 = { nounwind }
attributes #5 = { optsize }

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
!9 = !{!"p1 int", !10, i64 0}
!10 = !{!"any pointer", !5, i64 0}
