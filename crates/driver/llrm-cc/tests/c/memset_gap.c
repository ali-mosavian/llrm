#include <stdint.h>
#include <stdbool.h>
/* The `one = (Node){...}` of QuickrBasic's forloop: fields set among zeroed holes. */
typedef struct N N;
struct N {
    int kind;
    uint8_t type, sh, op;
    bool dbl;
    uint8_t conv, mode, nargs;
    bool val;
    N *a, *b, *next;
    uint16_t fix;
    uint32_t num;
    uint16_t id;
    uint64_t rec;
    uint8_t vtype, vinfo;
    bool par;
    uint32_t jump[3];
    uint8_t lit[16];
    const uint8_t *end;
};
void use(N *);
void f(N *one, int is_int)
{
    *one = (N){ .kind = 3, .op = is_int ? 5 : 6, .type = is_int ? 1 : 2, .lit = {1} };
    use(one);
}
