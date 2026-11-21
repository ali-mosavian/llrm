struct Node { unsigned char kind; };
extern void a(int);
extern void b(int);

void classify(struct Node *n)
{
    switch (n->kind) {
    case 130: a(1); break;
    case 140: b(2); break;
    case 150: a(3); break;
    case 200: b(4); break;
    case 210: a(5); break;
    }
}
