struct node { struct node *next; int v; };
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

int main(void)
{
    return sum(&head);
}
