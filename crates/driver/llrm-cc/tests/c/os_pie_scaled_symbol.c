#define N 1200
static int a[N], b[N];
static short c[N];

void fill(int v) {
    for (int i = 0; i < N; i++) {
        a[i] = v;
        b[i] = v;
        c[i] = 7;
    }
}

int pick(int p) { return a[p] + b[p] + c[p]; }
