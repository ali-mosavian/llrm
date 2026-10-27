/* The array descriptor on a host: the flat target's, whose counts, bounds and sizes are words of the target
   (`uword`), so a 64-bit host's are 8 bytes and the pointer slot is 8. The host's platform.h comes first so that
   the flat target's own, which it names, is skipped. */
#include "platform.h"
#include "../dos/m32/ad.h"
