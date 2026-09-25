typedef signed long long i64;
typedef unsigned long long u64;

i64 i64Add(i64 left, i64 right) {
    return left + right;
}

u64 u64Math(u64 left, u64 right, unsigned count) {
    return (left * right) / 3u + (left >> count);
}

int i64Less(i64 left, i64 right) {
    return left < right;
}

int u64Less(u64 left, u64 right) {
    return left < right;
}

i64 i64Extend(short value) {
    return (i64)value;
}

u64 u64Extend(unsigned short value) {
    return (u64)value;
}

long i64Narrow(i64 value) {
    return (long)value;
}

i64 i64Call(i64 value) {
    return i64Add(value, 1);
}

double i64ToDouble(i64 value) {
    return (double)value;
}

double u64ToDouble(u64 value) {
    return (double)value;
}

i64 doubleToI64(double value) {
    return (i64)value;
}

u64 doubleToU64(double value) {
    return (u64)value;
}
