target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"
target triple = "i386-pc-linux-gnu"

declare i32 @__clzsi2(i32) optsize
declare i32 @sf32_pack(i32, i32, i32) optsize
declare i32 @llvm.fshl.i32(i32, i32, i32) nounwind speculatable willreturn memory(none)

define i32 @__mulsf3(i32 noundef %0, i32 noundef %1) nounwind optsize {
  %3 = xor i32 %1, %0
  %4 = and i32 %3, -2147483648
  %5 = shl i32 %0, 1
  %6 = icmp ugt i32 %5, -16777217
  %7 = shl i32 %1, 1
  %8 = icmp ugt i32 %7, -16777217
  %9 = or i1 %6, %8
  br i1 %9, label %10, label %23

10:
  %11 = icmp ult i32 %5, -16777215
  br i1 %11, label %14, label %12

12:
  %13 = or i32 %0, 4194304
  br label %76

14:
  %15 = icmp ult i32 %7, -16777215
  br i1 %15, label %18, label %16

16:
  %17 = or i32 %1, 4194304
  br label %76

18:
  %19 = icmp eq i32 %5, 0
  %20 = icmp eq i32 %7, 0
  %or.cond = or i1 %19, %20
  %21 = or disjoint i32 %4, 2139095040
  %22 = select i1 %or.cond, i32 2143289344, i32 %21
  br label %76

23:
  %24 = icmp eq i32 %5, 0
  %25 = icmp eq i32 %7, 0
  %or.cond1 = or i1 %24, %25
  br i1 %or.cond1, label %76, label %26

26:
  %27 = and i32 %0, 8388607
  %28 = lshr i32 %0, 23
  %29 = and i32 %28, 255
  %30 = icmp eq i32 %29, 0
  %31 = or disjoint i32 %27, 8388608
  %32 = and i32 %1, 8388607
  %33 = lshr i32 %1, 23
  %34 = and i32 %33, 255
  %35 = or disjoint i32 %32, 8388608
  br i1 %30, label %36, label %41

36:
  %37 = tail call i32 @__clzsi2(i32 noundef %27) optsize
  %38 = add nsw i32 %37, -8
  %39 = shl i32 %27, %38
  %40 = sub nsw i32 9, %37
  br label %41

41:
  %42 = phi i32 [ %40, %36 ], [ %29, %26 ]
  %43 = phi i32 [ %31, %26 ], [ %39, %36 ]
  %44 = icmp eq i32 %34, 0
  br i1 %44, label %45, label %50

45:
  %46 = tail call i32 @__clzsi2(i32 noundef %32) optsize
  %47 = add nsw i32 %46, -8
  %48 = shl i32 %32, %47
  %49 = sub nsw i32 9, %46
  br label %50

50:
  %51 = phi i32 [ %49, %45 ], [ %34, %41 ]
  %52 = phi i32 [ %35, %41 ], [ %48, %45 ]
  %53 = zext i32 %43 to i64
  %54 = zext i32 %52 to i64
  %55 = mul nuw i64 %54, %53
  %56 = lshr i64 %55, 32
  %57 = trunc nuw i64 %56 to i32
  %58 = trunc i64 %55 to i32
  %59 = add nsw i32 %51, %42
  %60 = add nsw i32 %59, -126
  %61 = icmp ult i32 %57, 32768
  br i1 %61, label %62, label %66

62:
  %63 = add nsw i32 %59, -127
  %64 = tail call i32 @llvm.fshl.i32(i32 %57, i32 %58, i32 1)
  %65 = shl i32 %58, 1
  br label %66

66:
  %67 = phi i32 [ %58, %50 ], [ %65, %62 ]
  %68 = phi i32 [ %60, %50 ], [ %63, %62 ]
  %69 = phi i32 [ %57, %50 ], [ %64, %62 ]
  %70 = tail call i32 @llvm.fshl.i32(i32 %69, i32 %67, i32 11)
  %71 = and i32 %67, 2097151
  %72 = icmp ne i32 %71, 0
  %73 = zext i1 %72 to i32
  %74 = or i32 %70, %73
  %75 = tail call i32 @sf32_pack(i32 noundef %4, i32 noundef %68, i32 noundef %74) optsize
  ret i32 %75

76:
  %77 = phi i32 [ %13, %12 ], [ %17, %16 ], [ %22, %18 ], [ %4, %23 ]
  ret i32 %77
}
