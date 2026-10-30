#include "report.h"
struct node { long key; struct node *next; short tag; char flag; };
#ifdef __DOS__
_Static_assert(sizeof(struct node) == (sizeof(int) == 2 ? 9 : 12), "target layout");
#endif
static struct node pool[6];
static long sum(const struct node *n) { long s = 0; for (; n; n = n->next) s += n->key * n->tag + n->flag; return s; }
int main(void)
{
    for (int i = 0; i < 6; i++) {
        pool[i].key = 1000L * (i + 1);
        pool[i].tag = (short)(i - 2);
        pool[i].flag = (char)(i * 3);
        pool[i].next = i < 5 ? &pool[i + 1] : 0;
    }
    report(sum(pool));
    return 0;
}
