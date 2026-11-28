target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
@data = global [16 x i32] zeroinitializer, align 4
@flag = global i32 0, align 4
declare void @__atomic_store(i32, ptr, ptr, i32)
define void @publish(i32 %x) {
  %one = alloca i32, align 4
  store i32 1, ptr %one, align 4
  store i32 %x, ptr getelementptr inbounds nuw (i8, ptr @data, i32 4), align 2
  call void @__atomic_store(i32 4, ptr @flag, ptr %one, i32 3)
  ret void
}
