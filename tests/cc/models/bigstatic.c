char huge big[100000];
int main(void) { big[99999] = 1; return big[99999]; }
