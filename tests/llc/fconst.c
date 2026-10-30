#include <stdio.h>
#include <string.h>
#include <stdint.h>
volatile double vz = 0.0;
int main(void) {
    double c[6] = {0.0, -0.0, 1e300, 4.9e-324, 3.141592653589793, 1.0 / 3.0};
    float g[4] = {0.0f, 1e-45f, 3.4028235e38f, 0.1f};
    volatile double w[6]; volatile float h[4];
    for (int i = 0; i < 6; i++) w[i] = c[i];
    for (int i = 0; i < 4; i++) h[i] = g[i];
    unsigned long long bits[6]; uint32_t fb[4];
    double cc[6]; float gg[4];
    for (int i = 0; i < 6; i++) cc[i] = w[i];
    for (int i = 0; i < 4; i++) gg[i] = h[i];
    memcpy(bits, cc, sizeof cc); memcpy(fb, gg, sizeof gg);
    for (int i = 0; i < 6; i++) printf("%llx\n", bits[i] + (unsigned long long)vz);
    for (int i = 0; i < 4; i++) printf("%lx\n", (unsigned long)fb[i]);
    return 0;
}
