target datalayout = "@m32-datalayout@"
target triple = "i486-unknown-linux-gnu"
@g = internal global [2 x [24 x [24 x i8]]] zeroinitializer, align 1
define i32 @bench_x_life(i32 %0) {
  %2 = icmp sgt i32 %0, 0
  br i1 %2, label %3, label %7
3:
  %4 = phi i32 [ %21, %20 ], [ 0, %1 ]
  %5 = mul nuw nsw i32 %4, 13
  %6 = getelementptr inbounds nuw [24 x i8], ptr @g, i32 %4
  br label %10
7:
  %8 = add i32 %0, -1
  %9 = icmp sgt i32 %0, 2
  br label %23
10:
  %11 = phi i32 [ 0, %3 ], [ %18, %10 ]
  %12 = mul nuw nsw i32 %11, 7
  %13 = add nuw nsw i32 %12, %5
  %14 = urem i32 %13, 5
  %15 = icmp eq i32 %14, 0
  %16 = zext i1 %15 to i8
  %17 = getelementptr inbounds nuw i8, ptr %6, i32 %11
  store i8 %16, ptr %17, align 1
  %18 = add nuw nsw i32 %11, 1
  %19 = icmp eq i32 %18, %0
  br i1 %19, label %20, label %10
20:
  %21 = add nuw nsw i32 %4, 1
  %22 = icmp eq i32 %21, %0
  br i1 %22, label %7, label %3
23:
  %24 = phi i32 [ 0, %7 ], [ %75, %74 ]
  %25 = phi i32 [ 0, %7 ], [ %76, %74 ]
  br i1 %9, label %26, label %74
26:
  %27 = getelementptr inbounds nuw [576 x i8], ptr @g, i32 %24
  %28 = getelementptr inbounds nuw [576 x i8], ptr @g, i32 %24
  %29 = sub nuw nsw i32 1, %24
  %30 = getelementptr inbounds nuw [576 x i8], ptr @g, i32 %29
  br label %33
31:
  %32 = icmp sgt i32 %0, 0
  br i1 %32, label %78, label %94
33:
  %34 = phi i32 [ 1, %26 ], [ %72, %71 ]
  %35 = getelementptr [24 x i8], ptr %27, i32 %34
  %36 = getelementptr inbounds nuw [24 x i8], ptr %28, i32 %34
  %37 = getelementptr inbounds nuw [24 x i8], ptr %30, i32 %34
  br label %38
38:
  %39 = phi i32 [ 1, %33 ], [ %69, %66 ]
  %40 = getelementptr i8, ptr %35, i32 %39
  br label %41
41:
  %42 = phi i32 [ 0, %38 ], [ %51, %54 ]
  %43 = phi i32 [ -1, %38 ], [ %55, %54 ]
  %44 = getelementptr [24 x i8], ptr %40, i32 %43
  br label %45
45:
  %46 = phi i32 [ %42, %41 ], [ %51, %45 ]
  %47 = phi i32 [ -1, %41 ], [ %52, %45 ]
  %48 = getelementptr i8, ptr %44, i32 %47
  %49 = load i8, ptr %48, align 1
  %50 = zext i8 %49 to i32
  %51 = add nsw i32 %46, %50
  %52 = add nsw i32 %47, 1
  %53 = icmp eq i32 %52, 2
  br i1 %53, label %54, label %45
54:
  %55 = add nsw i32 %43, 1
  %56 = icmp eq i32 %55, 2
  br i1 %56, label %57, label %41
57:
  %58 = getelementptr inbounds nuw i8, ptr %36, i32 %39
  %59 = load i8, ptr %58, align 1
  %60 = zext i8 %59 to i32
  %61 = sub nsw i32 %51, %60
  switch i32 %61, label %65 [
    i32 3, label %66
    i32 2, label %62
  ]
62:
  %63 = icmp ne i8 %59, 0
  %64 = zext i1 %63 to i8
  br label %66
65:
  br label %66
66:
  %67 = phi i8 [ 1, %57 ], [ %64, %62 ], [ 0, %65 ]
  %68 = getelementptr inbounds nuw i8, ptr %37, i32 %39
  store i8 %67, ptr %68, align 1
  %69 = add nuw nsw i32 %39, 1
  %70 = icmp eq i32 %69, %8
  br i1 %70, label %71, label %38
71:
  %72 = add nuw nsw i32 %34, 1
  %73 = icmp eq i32 %72, %8
  br i1 %73, label %74, label %33
74:
  %75 = xor i32 %24, 1
  %76 = add nuw nsw i32 %25, 1
  %77 = icmp eq i32 %76, 10
  br i1 %77, label %31, label %23
78:
  %79 = phi i32 [ %88, %91 ], [ 0, %31 ]
  %80 = phi i32 [ %92, %91 ], [ 0, %31 ]
  %81 = getelementptr inbounds nuw [24 x i8], ptr @g, i32 %80
  br label %82
82:
  %83 = phi i32 [ %79, %78 ], [ %88, %82 ]
  %84 = phi i32 [ 0, %78 ], [ %89, %82 ]
  %85 = getelementptr inbounds nuw i8, ptr %81, i32 %84
  %86 = load i8, ptr %85, align 1
  %87 = zext i8 %86 to i32
  %88 = add nsw i32 %83, %87
  %89 = add nuw nsw i32 %84, 1
  %90 = icmp eq i32 %89, %0
  br i1 %90, label %91, label %82
91:
  %92 = add nuw nsw i32 %80, 1
  %93 = icmp eq i32 %92, %0
  br i1 %93, label %94, label %78
94:
  %95 = phi i32 [ 0, %31 ], [ %88, %91 ]
  ret i32 %95
}
