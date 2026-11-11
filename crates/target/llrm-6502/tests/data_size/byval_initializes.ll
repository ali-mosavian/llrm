target triple = "msp430"

%struct.big = type { [10 x i32] }

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca %struct.big, align 1
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  %3 = getelementptr inbounds nuw %struct.big, ptr %2, i32 0, i32 0
  %4 = getelementptr inbounds [10 x i32], ptr %3, i16 0, i16 0
  store i32 1, ptr %4, align 1, !tbaa !7
  %5 = call i32 @bump(ptr noundef byval(%struct.big) align 1 %2) #4
  call void @report(i32 noundef %5) #4
  %6 = getelementptr inbounds nuw %struct.big, ptr %2, i32 0, i32 0
  %7 = getelementptr inbounds [10 x i32], ptr %6, i16 0, i16 0
  %8 = load i32, ptr %7, align 1, !tbaa !7
  call void @report(i32 noundef %8) #4
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind optsize
define internal i32 @bump(ptr noundef byval(%struct.big) align 1 %0) #0 {
  %2 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 0
  %3 = getelementptr inbounds [10 x i32], ptr %2, i16 0, i16 0
  store i32 99, ptr %3, align 1, !tbaa !7
  %4 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 0
  %5 = getelementptr inbounds [10 x i32], ptr %4, i16 0, i16 0
  %6 = load i32, ptr %5, align 1, !tbaa !7
  ret i32 %6
}

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
