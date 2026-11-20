target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
target triple = "msp430"

define dso_local i32 @wrap(i16 noundef %0) nounwind {
  %2 = tail call i16 @step(i16 noundef %0)
  %3 = sext i16 %2 to i32
  ret i32 %3
}

declare dso_local i16 @step(i16 noundef)

define dso_local i16 @main() nounwind {
  %1 = call i32 @wrap(i16 noundef 20)
  call void @report(i32 noundef %1)
  ret i16 0
}

declare dso_local void @report(i32 noundef)
