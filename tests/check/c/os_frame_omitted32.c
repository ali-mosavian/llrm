// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -Os --isel=dag -S -o /dev/stdout
// At -Os masm kept `push ebp; mov ebp, esp` for a function whose frame register hasFP had freed, because the esp form was longer in
// bytes (BASIC's rule). The pin omits it whatever the bytes: QCport weapons.c -Os kept 17 `leave` where the pin has none.
// CHECK-LABEL: _probe proc
// CHECK-NOT: mov ebp, esp
// CHECK: _probe endp
extern double sqrt(double), atan2(double, double), cos(double);
float probe(float *p, float *q)
{
    float ax = q[0] - p[0], ay = q[1] - p[1], az = q[2] - p[2];
    float h = (float) sqrt(ax * ax + ay * ay);
    float t = (float) atan2(az, h) / 3.0f;
    float r[4];
    r[0] = ax / h * (float) cos(t);
    r[1] = ay / h * (float) cos(t);
    r[2] = az * (float) cos(t);
    r[3] = h;
    return r[0] + r[1] + r[2] + r[3];
}
