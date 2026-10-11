/* Three loads added in a loop with the registers all taken: the third load
   lands in ebp, which this function does not use as a frame. */
typedef struct {
    unsigned short length;
    unsigned short capacity;
    short *data;
} Slice;

short sum_three(Slice *first, Slice *second, Slice *third)
{
    unsigned short index;
    short total;

    total = 0;
    for (index = 0; index < first->length; ++index) {
        total += first->data[index];
        total += second->data[index];
        total += third->data[index];
    }
    return total;
}
