; ModuleID = '/tmp/tmp.8DN3fkB8Vi/sw.c'
source_filename = "/tmp/tmp.8DN3fkB8Vi/sw.c"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

; Function Attrs: nounwind
define hidden void @f(i32 noundef %0) local_unnamed_addr #0 {
  switch i32 %0, label %10 [
    i32 0, label %2
    i32 1, label %3
    i32 2, label %4
    i32 3, label %5
    i32 4, label %6
    i32 5, label %7
    i32 6, label %8
    i32 7, label %9
  ]

2:                                                ; preds = %1
  tail call void @a(i32 noundef 3) #2
  br label %10

3:                                                ; preds = %1
  tail call void @b() #2
  br label %10

4:                                                ; preds = %1
  tail call void @a(i32 noundef 17) #2
  br label %10

5:                                                ; preds = %1
  tail call void @b() #2
  tail call void @a(i32 noundef 1) #2
  br label %10

6:                                                ; preds = %1
  tail call void @a(i32 noundef 31) #2
  br label %10

7:                                                ; preds = %1
  tail call void @b() #2
  br label %10

8:                                                ; preds = %1
  tail call void @a(i32 noundef 45) #2
  br label %10

9:                                                ; preds = %1
  tail call void @a(i32 noundef 52) #2
  tail call void @b() #2
  br label %10

10:                                               ; preds = %1, %9, %8, %7, %6, %5, %4, %3, %2
  ret void
}

declare void @a(i32 noundef) local_unnamed_addr #1

declare void @b() local_unnamed_addr #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #1 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #2 = { nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 4}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
