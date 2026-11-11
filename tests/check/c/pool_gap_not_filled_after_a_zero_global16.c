// RUN: llrm-cc %s -Os -march=i486 -fno-inline-functions -S -o /dev/stdout
// A global of zeros moves to .data? after the pool is laid out, so the pool's start offset is unknown: the float constant took a pad that was not there, and the double padded 4 bytes after it (uint64_to_float -O0: +4 B). With a zero global no gap is filled.
long g;
long read(void) { return g; }
double scale(double x) { return x * 0.01; }
float cent(float x) { return x * 100.0f; }
// CHECK: .data
// CHECK: db 07bh,014h,0aeh,047h,0e1h,07ah,084h,03fh
// CHECK-NOT: align
// CHECK: db 000h,000h,0c8h,042h
