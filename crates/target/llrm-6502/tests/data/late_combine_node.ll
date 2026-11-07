target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"

@__r_s = internal unnamed_addr global [48 x i16] zeroinitializer, align 2
@t = internal global [49 x i16] zeroinitializer, align 2
@v = internal unnamed_addr global [48 x i8] zeroinitializer, align 1
@d = internal global [48 x i16] zeroinitializer, align 2
@w.0 = internal global i8 0, align 1
@w.1 = internal global i8 0, align 1
@w.2 = internal global i8 0, align 1

define i16 @main() nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" {
  br label %1

1:
  %2 = phi i16 [ 0, %0 ], [ %12, %1 ]
  %3 = mul nuw nsw i16 %2, 7
  %4 = add nuw nsw i16 %3, 1
  %5 = getelementptr inbounds nuw [2 x i8], ptr @__r_s, i16 %2
  %6 = mul nuw nsw i16 %2, 5
  %7 = add nuw nsw i16 %6, 2
  %8 = getelementptr inbounds nuw [2 x i8], ptr @t, i16 %2
  %9 = trunc nuw i16 %2 to i8
  %10 = add i8 %9, -20
  %11 = getelementptr inbounds nuw i8, ptr @v, i16 %2
  %12 = add nuw nsw i16 %2, 1
  %13 = icmp samesign ult i16 %2, 47
  br i1 %13, label %1, label %14, !llvm.loop !11

14:
  %15 = load i16, ptr @__r_s, align 2, !tbaa !8
  %16 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 2), align 2, !tbaa !8
  %17 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 4), align 2, !tbaa !8
  %18 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 6), align 2, !tbaa !8
  %19 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 8), align 2, !tbaa !8
  %20 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 10), align 2, !tbaa !8
  %21 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 12), align 2, !tbaa !8
  %22 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 14), align 2, !tbaa !8
  %23 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 16), align 2, !tbaa !8
  %24 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 18), align 2, !tbaa !8
  %25 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 20), align 2, !tbaa !8
  %26 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 22), align 2, !tbaa !8
  %27 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 24), align 2, !tbaa !8
  %28 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 26), align 2, !tbaa !8
  %29 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 28), align 2, !tbaa !8
  %30 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 30), align 2, !tbaa !8
  %31 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 32), align 2, !tbaa !8
  %32 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 34), align 2, !tbaa !8
  %33 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 36), align 2, !tbaa !8
  %34 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 38), align 2, !tbaa !8
  %35 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 40), align 2, !tbaa !8
  %36 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 42), align 2, !tbaa !8
  %37 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 44), align 2, !tbaa !8
  %38 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 46), align 2, !tbaa !8
  %39 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 48), align 2, !tbaa !8
  %40 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 50), align 2, !tbaa !8
  %41 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 52), align 2, !tbaa !8
  %42 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 54), align 2, !tbaa !8
  %43 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 56), align 2, !tbaa !8
  %44 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 58), align 2, !tbaa !8
  %45 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 60), align 2, !tbaa !8
  %46 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 62), align 2, !tbaa !8
  %47 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 64), align 2, !tbaa !8
  %48 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 66), align 2, !tbaa !8
  %49 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 68), align 2, !tbaa !8
  %50 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 70), align 2, !tbaa !8
  %51 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 72), align 2, !tbaa !8
  %52 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 74), align 2, !tbaa !8
  %53 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 76), align 2, !tbaa !8
  %54 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 78), align 2, !tbaa !8
  %55 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 80), align 2, !tbaa !8
  %56 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 82), align 2, !tbaa !8
  %57 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 84), align 2, !tbaa !8
  %58 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 86), align 2, !tbaa !8
  %59 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 88), align 2, !tbaa !8
  %60 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 90), align 2, !tbaa !8
  %61 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 92), align 2, !tbaa !8
  %62 = load i16, ptr getelementptr inbounds nuw (i8, ptr @__r_s, i16 94), align 2, !tbaa !8
  %63 = load i16, ptr @t, align 2, !tbaa !8
  store i16 %63, ptr getelementptr inbounds nuw (i8, ptr @t, i16 96), align 2, !tbaa !8
  %64 = load i8, ptr @v, align 1, !tbaa !10
  %65 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 17), align 1, !tbaa !10
  %66 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 47), align 1, !tbaa !10
  br label %67

67:
  %68 = phi i16 [ 0, %14 ], [ %76, %67 ]
  %69 = phi i32 [ 0, %14 ], [ %75, %67 ]
  %70 = mul nsw i32 %69, 3
  %71 = getelementptr inbounds nuw [2 x i8], ptr @d, i16 %68
  %72 = load i16, ptr %71, align 2, !tbaa !8
  %73 = sext i16 %72 to i32
  %74 = add nsw i32 %70, %73
  %75 = srem i32 %74, 1000003
  %76 = add nuw nsw i16 %68, 1
  %77 = icmp samesign ult i16 %68, 47
  br i1 %77, label %67, label %78, !llvm.loop !19

78:
  br label %79

79:
  %80 = phi i16 [ 0, %78 ], [ %88, %79 ]
  %81 = phi i32 [ 0, %78 ], [ %87, %79 ]
  %82 = mul nsw i32 %81, 3
  %83 = getelementptr inbounds nuw [2 x i8], ptr @t, i16 %80
  %84 = load i16, ptr %83, align 2, !tbaa !8
  %85 = sext i16 %84 to i32
  %86 = add nsw i32 %82, %85
  %87 = srem i32 %86, 1000003
  %88 = add nuw nsw i16 %80, 1
  %89 = icmp samesign ult i16 %80, 48
  br i1 %89, label %79, label %90, !llvm.loop !19

90:
  tail call void @report(i32 noundef %87)
  ret i16 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @report(i32 noundef) "no-trapping-math"="true" "stack-protector-buffer-size"="8"

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

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
!9 = !{!"short", !5, i64 0}
!10 = !{!5, !5, i64 0}
!11 = distinct !{!11, !12, !13}
!12 = !{!"llvm.loop.mustprogress"}
!13 = !{!"llvm.loop.unroll.disable"}
!14 = !{!15, !15, i64 0}
!15 = !{!"p1 short", !16, i64 0}
!16 = !{!"any pointer", !5, i64 0}
!17 = !{!18, !18, i64 0}
!18 = !{!"long", !5, i64 0}
!19 = distinct !{!19, !12, !13}
