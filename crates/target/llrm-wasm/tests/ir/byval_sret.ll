%struct.pair = type { i64, i64 }

define void @swap(ptr noalias sret(%struct.pair) align 8 %r, ptr byval(%struct.pair) align 8 %v) {
entry:
  %lo = load i64, ptr %v, align 8
  %hip = getelementptr inbounds %struct.pair, ptr %v, i32 0, i32 1
  %hi = load i64, ptr %hip, align 8
  %rhp = getelementptr inbounds %struct.pair, ptr %r, i32 0, i32 1
  store i64 %lo, ptr %rhp, align 8
  store i64 %hi, ptr %r, align 8
  ret void
}

define i64 @use(i64 %a, i64 %b) {
entry:
  %in = alloca %struct.pair, align 8
  %out = alloca %struct.pair, align 8
  store i64 %a, ptr %in, align 8
  %bp = getelementptr inbounds %struct.pair, ptr %in, i32 0, i32 1
  store i64 %b, ptr %bp, align 8
  call void @swap(ptr sret(%struct.pair) align 8 %out, ptr byval(%struct.pair) align 8 %in)
  %v = load i64, ptr %out, align 8
  ret i64 %v
}
