typedef struct {
    unsigned short length;
    unsigned short capacity;
    short far *data;
} Slice;

short sum_three(Slice far *first, Slice far *second, Slice far *third)
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

static short first_data[4] = {1, 2, 3, 4};
static short second_data[4] = {10, 20, 30, 40};
static short third_data[4] = {100, 200, 300, 400};

long sum_three_demo(void)
{
    Slice first;
    Slice second;
    Slice third;

    first.length = 4;
    first.capacity = 4;
    first.data = first_data;
    second.length = 4;
    second.capacity = 4;
    second.data = second_data;
    third.length = 4;
    third.capacity = 4;
    third.data = third_data;
    return sum_three(&first, &second, &third);
}
