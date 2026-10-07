struct point { int x; int y; };
struct node { struct node *next; int v; };
typedef struct point pt_t;
int count;
static long total;
struct point origin;
struct node tail = { 0, 7 };
struct node head = { &tail, 3 };
int sum(struct node *n)
{
    int s = 0;
    while (n) {
        s += n->v;
        n = n->next;
    }
    return s;
}
int f(int a, int b)
{
    int x = a + b;
    struct point q;
    q.x = x;
    count = q.x;
    total += x;
    return x;
}
int main(void)
{
    return sum(&head) + f(1, 2);
}
int isatty(int h) { return 0; }
