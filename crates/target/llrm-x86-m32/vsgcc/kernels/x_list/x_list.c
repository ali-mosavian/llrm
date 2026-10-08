extern void report(long value);

struct node { struct node *next; int v; };
static struct node pool[256];
long bench_x_list(int n)
{
    int i, r; long sum = 0; struct node *head = 0, *p, *prev, *next;
    for (i = 0; i < n; ++i) { pool[i].v = (i * 37) & 255; pool[i].next = head; head = &pool[i]; }
    for (r = 0; r < 10; ++r) {
        prev = 0; p = head;
        while (p) { next = p->next; p->next = prev; prev = p; p = next; }
        head = prev;
        for (p = head; p; p = p->next) sum += p->v * (r + 1);
    }
    return sum;
}

int main(void)
{
    report(bench_x_list(200));
    return 0;
}
