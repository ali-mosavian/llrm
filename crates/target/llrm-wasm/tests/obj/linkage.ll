@str = private unnamed_addr constant [4 x i8] c"abc\00", align 1
@cnt = internal global i32 3, align 4
@ptr = global ptr @cnt, align 4
@ext = external global i32
declare void @take(ptr)
define i32 @f() {
  %a = alloca [4 x i32], align 4
  call void @take(ptr %a)
  call void @take(ptr @str)
  %v = load i32, ptr @cnt
  %w = load i32, ptr @ext
  %s = add i32 %v, %w
  ret i32 %s
}
