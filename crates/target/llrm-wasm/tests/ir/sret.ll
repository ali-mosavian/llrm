%struct.T = type { i32, i32, i16 }
define void @make(ptr sret(%struct.T) align 4 %out, i32 %n) {
  store i32 %n, ptr %out, align 4
  %b = getelementptr inbounds %struct.T, ptr %out, i32 0, i32 1
  store i32 7, ptr %b, align 4
  %c = getelementptr inbounds %struct.T, ptr %out, i32 0, i32 2
  store i16 3, ptr %c, align 2
  ret void
}
define i32 @use(i32 %n) {
  %t = alloca %struct.T, align 4
  call void @make(ptr sret(%struct.T) align 4 %t, i32 %n)
  %b = getelementptr inbounds %struct.T, ptr %t, i32 0, i32 1
  %v = load i32, ptr %b, align 4
  ret i32 %v
}
