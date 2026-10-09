/* String space and the temporary descriptors (QB rt/nhstutil.asm, stcore.asm). */
#ifndef QB_NHSTUTIL_H
#define QB_NHSTUTIL_H

#include "nheap.h"

/* The zero-length string's descriptor, which every empty result shares. */
extern SD str_nul;

word str_alloc(word len);
SD *str_tmp(word len, word *data);
SD *str_tmp_sub(SD *src, word off, word len);
void str_tmp_free(SD *sd);
void str_free_sd(SD *sd);
void str_tmp_release(SD *sd);
byte str_is_tmp(SD *sd);
void str_adjust(word sd, word delta);
void str_all_tmp_free(word level);
extern word cur_level;

#endif
