/* B$RDFS: READ into a fixed-length string. */
#include "fixstr.h"
#include "nhstutil.h"
#include "read.h"

#pragma aux B_RDFS "B$RDFS"

/* The item as a string cut or padded with blanks to the `width` bytes at `destination`. It is read as a variable
   string into a descriptor of our own, which the free of its string ends; B$RDSD stays what it was. */
void B_RDFS(qb_data_ptr destination, short width)
{
    SD item = {0, 0};

    B_RDSD(QB_FAR_OF(&item), 0);
    fixed_assign(QB_NEAR_OF(destination), item.ptr, item.len, width);
    str_release(&item);
}
