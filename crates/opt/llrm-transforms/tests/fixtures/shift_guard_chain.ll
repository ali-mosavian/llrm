define i32 @f(i32 %0, i32 %1) {
  %3 = icmp eq i32 %1, 0
  br i1 %3, label %80, label %4

4:
  %5 = icmp ult i32 %1, 65536
  %6 = shl i32 %1, 16
  %7 = select i1 %5, i32 %6, i32 %1
  %8 = select i1 %5, i32 16, i32 0
  %9 = icmp ult i32 %7, 16777216
  %10 = or disjoint i32 %8, 8
  %11 = shl i32 %7, 8
  %12 = select i1 %9, i32 %11, i32 %7
  %13 = select i1 %9, i32 %10, i32 %8
  %14 = icmp ult i32 %12, 268435456
  %15 = or disjoint i32 %13, 4
  %16 = shl i32 %12, 4
  %17 = select i1 %14, i32 %16, i32 %12
  %18 = select i1 %14, i32 %15, i32 %13
  %19 = icmp ult i32 %17, 1073741824
  %20 = or disjoint i32 %18, 2
  %21 = shl i32 %17, 2
  %22 = select i1 %19, i32 %21, i32 %17
  %23 = select i1 %19, i32 %20, i32 %18
  %24 = icmp sgt i32 %22, -1
  %25 = zext i1 %24 to i32
  %26 = add nuw nsw i32 %23, %25
  %27 = xor i32 %26, 31
  %28 = icmp samesign ugt i32 %27, 26
  br i1 %28, label %29, label %41

29:
  %30 = sub nsw i32 5, %26
  %31 = icmp slt i32 %30, 1
  br i1 %31, label %44, label %32

32:
  %33 = icmp sgt i32 %30, 31
  br i1 %33, label %44, label %34

34:
  %35 = lshr i32 %1, %30
  %36 = sub nuw nsw i32 32, %30
  %37 = shl i32 %1, %36
  %38 = icmp ne i32 %37, 0
  %39 = zext i1 %38 to i32
  %40 = or i32 %35, %39
  br label %44

41:
  %42 = sub nuw nsw i32 26, %27
  %43 = shl i32 %1, %42
  br label %44

44:
  %45 = phi i32 [ %43, %41 ], [ %40, %34 ], [ %1, %29 ], [ 1, %32 ]
  %46 = sub nuw nsw i32 158, %26
  %47 = icmp sgt i32 %46, 254
  br i1 %47, label %48, label %50

48:
  %49 = or i32 %0, 2139095040
  br label %80

50:
  %51 = icmp slt i32 %46, 1
  br i1 %51, label %52, label %65

52:
  %53 = icmp slt i32 %46, -30
  br i1 %53, label %54, label %57

54:
  %55 = icmp ne i32 %45, 0
  %56 = zext i1 %55 to i32
  br label %65

57:
  %58 = sub nsw i32 1, %46
  %59 = lshr i32 %45, %58
  %60 = add nsw i32 %46, 31
  %61 = shl i32 %45, %60
  %62 = icmp ne i32 %61, 0
  %63 = zext i1 %62 to i32
  %64 = or i32 %59, %63
  br label %65

65:
  %66 = phi i32 [ %46, %50 ], [ 1, %57 ], [ 1, %54 ]
  %67 = phi i32 [ %45, %50 ], [ %56, %54 ], [ %64, %57 ]
  %68 = and i32 %67, 7
  %69 = lshr i32 %67, 3
  %70 = icmp samesign ugt i32 %68, 4
  %71 = icmp eq i32 %68, 4
  %72 = trunc i32 %69 to i1
  %73 = and i1 %71, %72
  %or.cond1 = or i1 %70, %73
  %74 = zext i1 %or.cond1 to i32
  %75 = shl nuw nsw i32 %66, 23
  %76 = add i32 %0, -8388608
  %77 = add i32 %76, %75
  %78 = add i32 %77, %69
  %79 = add i32 %78, %74
  br label %80

80:
  %81 = phi i32 [ 0, %2 ], [ %49, %48 ], [ %79, %65 ]
  ret i32 %81
}

