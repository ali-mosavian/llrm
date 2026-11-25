/* Two external wrappers whose body is one call with constant arguments. At -Os the
   call to a wrapper (2 arguments) is smaller than its inlined body (a 3-argument
   call plus a store), and the wrapper stays, so each inline only adds bytes. */
typedef struct Node {
    int pad[11];
    unsigned num;
} Node;

Node *node(void *c, int kind, int arg);

void wrap_int(void *c, unsigned n) { node(c, 2, 0)->num = n; }

Node *wrap_op(void *c, int op) { return node(c, 1, op); }

void user(void *c, int x)
{
    if (x)
        wrap_int(c, x + 65536u);
    else
        wrap_op(c, 3);
}

/* A static callee that goes with its only call is still inlined. */
static Node *inner(void *c, int op) { return node(c, 5, op); }

void user_dies(void *c) { inner(c, 7); }
