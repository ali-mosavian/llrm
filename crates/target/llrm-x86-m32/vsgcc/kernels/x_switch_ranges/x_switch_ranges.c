extern void report(long value);

static int class_of(int c)
{
    switch (c) {
    case 48: case 49: case 50: case 51: case 52: case 53: case 54: case 55: case 56: case 57: return 1;
    case 97: case 98: case 99: case 100: case 101: case 102: return 2;
    case 103: case 104: case 105: case 106: case 107: case 108: case 109: case 110: case 111: case 112: case 113: case 114: case 115: case 116: case 117: case 118: case 119: case 120: case 121: case 122: return 3;
    case 65: case 66: case 67: case 68: case 69: case 70: case 71: case 72: case 73: case 74: case 75: case 76: case 77: case 78: case 79: case 80: case 81: case 82: case 83: case 84: case 85: case 86: case 87: case 88: case 89: case 90: return 4;
    case 32: case 9: case 10: return 5;
    case 95: return 6;
    default: return 0;
    }
}
long bench_x_switch_ranges(int n)
{
    int i; long sum = 0; unsigned r = 3;
    for (i = 0; i < n; ++i) { r = r * 1103515245u + 12345u; sum += class_of((int)((r >> 16) & 127)); }
    return sum;
}

int main(void)
{
    report(bench_x_switch_ranges(2000));
    return 0;
}
