extern int g;
extern int arr[8];

int *address_of_g(void) { return &g; }
int *address_of(int i) { return &arr[i]; }
