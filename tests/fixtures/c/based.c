/* Based pointers: a near offset in a segment named by __segname. */
char x = 42;
char __based(__segname("_DATA")) *dp;
char __based(__segname("_CODE")) *cp;

#pragma data_seg("TABLES")
char table[4] = { 5, 6, 7, 8 };
#pragma data_seg()
char __based(__segname("TABLES")) *tp;

int read_data(void)
{
    dp = (char __based(__segname("_DATA")) *)&x;
    return *dp;
}

int read_code(void) { return *cp; }
int read_table(void) { return tp[2]; }
