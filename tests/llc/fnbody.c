#include <stdio.h>
#include <math.h>
typedef struct { double x, y, z, vx, vy, vz, m; } Body;
static void step(Body *b, int n, double dt) {
    for (int i = 0; i < n; i++)
        for (int j = i + 1; j < n; j++) {
            double dx = b[i].x - b[j].x, dy = b[i].y - b[j].y, dz = b[i].z - b[j].z;
            double d2 = dx * dx + dy * dy + dz * dz + 0.01;
            double mag = dt / (d2 * sqrt(d2));
            b[i].vx -= dx * b[j].m * mag; b[i].vy -= dy * b[j].m * mag; b[i].vz -= dz * b[j].m * mag;
            b[j].vx += dx * b[i].m * mag; b[j].vy += dy * b[i].m * mag; b[j].vz += dz * b[i].m * mag;
        }
    for (int i = 0; i < n; i++) { b[i].x += dt * b[i].vx; b[i].y += dt * b[i].vy; b[i].z += dt * b[i].vz; }
}
int main(void) {
    Body b[4] = {{0, 0, 0, 0, 0, 0, 10}, {1, 0, 0, 0, 1, 0, 1}, {0, 2, 0, -1, 0, 0, 0.5}, {3, 3, 1, 0, 0, 0.2, 0.1}};
    for (int s = 0; s < 1000; s++) step(b, 4, 0.01);
    for (int i = 0; i < 4; i++) printf("%ld %ld %ld\n", (long)(b[i].x * 1e3), (long)(b[i].y * 1e3), (long)(b[i].z * 1e3));
    return 0;
}
