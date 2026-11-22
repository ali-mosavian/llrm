/* An interpreter's dispatch: twenty handlers `goto *` the next one; the handlers differ, so nothing merges them. */
long count;
long hits[20];
long run(const unsigned char *pc, long *sp)
{
    static const void *const tab[] = { &&op0, &&op1, &&op2, &&op3, &&op4, &&op5, &&op6, &&op7, &&op8, &&op9, &&op10, &&op11, &&op12, &&op13, &&op14, &&op15, &&op16, &&op17, &&op18, &&op19 };
#define NEXT do { count++; goto *tab[*pc]; } while (0)
    NEXT;
op0: hits[0]++; sp[0] = sp[-1] + (sp[-2] + 3); sp++; pc += 1 + (0&1); NEXT;
op1: hits[1]++; sp[0] = sp[-1] - (sp[-2] + 10); sp++; pc += 1 + (1&1); NEXT;
op2: hits[2]++; sp[0] = sp[-1] * (sp[-2] + 17); sp++; pc += 1 + (2&1); NEXT;
op3: hits[3]++; sp[0] = sp[-1] ^ (sp[-2] + 24); sp++; pc += 1 + (3&1); NEXT;
op4: hits[4]++; sp[0] = sp[-1] | (sp[-2] + 31); sp++; pc += 1 + (4&1); NEXT;
op5: hits[5]++; sp[0] = sp[-1] & (sp[-2] + 38); sp++; pc += 1 + (5&1); NEXT;
op6: hits[6]++; sp[0] = sp[-1] + (sp[-2] + 45); sp++; pc += 1 + (6&1); NEXT;
op7: hits[7]++; sp[0] = sp[-1] - (sp[-2] + 52); sp++; pc += 1 + (7&1); NEXT;
op8: hits[8]++; sp[0] = sp[-1] * (sp[-2] + 59); sp++; pc += 1 + (8&1); NEXT;
op9: hits[9]++; sp[0] = sp[-1] ^ (sp[-2] + 66); sp++; pc += 1 + (9&1); NEXT;
op10: hits[10]++; sp[0] = sp[-1] | (sp[-2] + 73); sp++; pc += 1 + (10&1); NEXT;
op11: hits[11]++; sp[0] = sp[-1] & (sp[-2] + 80); sp++; pc += 1 + (11&1); NEXT;
op12: hits[12]++; sp[0] = sp[-1] + (sp[-2] + 87); sp++; pc += 1 + (12&1); NEXT;
op13: hits[13]++; sp[0] = sp[-1] - (sp[-2] + 94); sp++; pc += 1 + (13&1); NEXT;
op14: hits[14]++; sp[0] = sp[-1] * (sp[-2] + 101); sp++; pc += 1 + (14&1); NEXT;
op15: hits[15]++; sp[0] = sp[-1] ^ (sp[-2] + 108); sp++; pc += 1 + (15&1); NEXT;
op16: hits[16]++; sp[0] = sp[-1] | (sp[-2] + 115); sp++; pc += 1 + (16&1); NEXT;
op17: hits[17]++; sp[0] = sp[-1] & (sp[-2] + 122); sp++; pc += 1 + (17&1); NEXT;
op18: hits[18]++; sp[0] = sp[-1] + (sp[-2] + 129); sp++; pc += 1 + (18&1); NEXT;
op19: return sp[-1];
}
