#include "report.h"
static unsigned char flags[8192];
int main(void)
{
    int count = 0;
    for (int i = 2; i < 8192; i++) {
        if (flags[i]) continue;
        count++;
        for (int j = i * 2; j < 8192; j += i) flags[j] = 1;
    }
    report(count);
    return 0;
}
