/* A based pointer into a segment this unit places nothing in. */
char __based(__segname("ELSEWHERE")) *ep;
int read_elsewhere(void) { return *ep; }
