// RUN: llrm-cc %s -m32 -mabi=sysv -Os -march=i486 -S -o /dev/stdout
// A dword global leaves the data at 4 mod 8, so the 8-aligned double constant padded 4 bytes and the float constant followed it (x_horner -Os m32: data 84 B, gcc 80). The float belongs in the pad: it comes first.
int g = 3;
double scale(double x) { return x * 0.01; }
float cent(float x) { return x * 100.0f; }
// CHECK: db 000h,000h,0c8h,042h
// CHECK: db 07bh,014h,0aeh,047h,0e1h,07ah,084h,03fh
