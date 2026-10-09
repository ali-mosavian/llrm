/* QB dynamic-array descriptors and their reusable near allocation arena. */

#include "types.h"

typedef struct {
    word count;
    word lower;
} dimension;

typedef struct {
    word data;
    word segment;
    word next;
    word bytes;
    byte dimensions;
    byte features;
    word adjusted;
    word element;
    dimension dimension[1];
} array;

typedef struct block {
    word next;
    word bytes;
    word data;
} block;

extern byte *__near qb_array_more(word bytes);

static word free_blocks;
word qb_array_bounds;

static word qb_array_address_of(const void *pointer) {
    return (word)pointer;
}

static block *block_at(word address) {
    return (block *)address;
}

static block *qb_array_allocate(word bytes) {
    block *record = block_at(free_blocks);

    while (record != (block *)0) {
        if (record->bytes >= bytes) {
            free_blocks = record->next;
            return record;
        }
        record = block_at(record->next);
    }

    if (bytes > 65535U - sizeof(block) - 15U) {
        return (block *)0;
    }
    record = (block *)qb_array_more(bytes + sizeof(block) + 15U);
    if (record == (block *)0) {
        return (block *)0;
    }
    record->next = 0;
    record->bytes = bytes;
    record->data = (qb_array_address_of(record + 1) + 15U) & (word)~15U;
    return record;
}

static void qb_array_clear(array *value) {
    value->data = 0;
    value->segment = 0;
    value->next = 0;
    value->bytes = 0;
    value->dimensions = 0;
    value->features = 0;
    value->adjusted = 0;
    value->element = 0;
}

word __near qb_array_dim(word array_address, word type_and_dimensions, word element_bytes) {
    array *value = (array *)array_address;
    const word *bounds = (const word *)qb_array_bounds;
    word dimensions = type_and_dimensions & 255;
    word index;
    word count;
    sword upper;
    sword lower;
    sword adjustment = 0;
    dword total = 1;
    dword byte_count;
    block *record;
    byte *data;

    qb_array_clear(value);
    if (dimensions == 0 || element_bytes == 0) {
        return 0;
    }

    for (index = 0; index < dimensions; ++index) {
        upper = (sword)bounds[index * 2];
        lower = (sword)bounds[index * 2 + 1];
        if (upper < lower) {
            qb_array_clear(value);
            return 0;
        }
        count = (word)(upper - lower + 1);
        total *= count;
        if (total > 65535UL) {
            qb_array_clear(value);
            return 0;
        }
        adjustment = adjustment * count - lower;
        value->dimension[index].count = count;
        value->dimension[index].lower = lower;
    }

    byte_count = total * element_bytes;
    if (byte_count == 0 || byte_count > 65535UL) {
        qb_array_clear(value);
        return 0;
    }

    record = qb_array_allocate((word)byte_count);
    if (record == (block *)0) {
        qb_array_clear(value);
        return 0;
    }
    data = (byte *)record->data;

    value->data = qb_array_address_of(data);
    value->next = qb_array_address_of(record);
    value->bytes = (word)byte_count;
    value->dimensions = (byte)dimensions;
    value->features = (byte)(type_and_dimensions >> 8);
    value->adjusted = adjustment * element_bytes + value->data;
    value->element = element_bytes;
    return 1;
}

void __near qb_array_erase(word array_address) {
    array *value = (array *)array_address;
    block *record;

    if (value->data == 0) {
        qb_array_clear(value);
        return;
    }

    record = block_at(value->next);
    record->next = free_blocks;
    free_blocks = qb_array_address_of(record);
    qb_array_clear(value);
}

word __near qb_array_redim(word array_address, word type_and_dimensions, word element_bytes) {
    qb_array_erase(array_address);
    return qb_array_dim(array_address, type_and_dimensions, element_bytes);
}
