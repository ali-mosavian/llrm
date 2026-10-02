/* -g of bit fields: each field's width and first bit reach CodeView. */
struct flags {
    unsigned a : 3;
    int b : 5;
    unsigned c : 9;
};

struct flags gf;

int get_b(void) { return gf.b; }
