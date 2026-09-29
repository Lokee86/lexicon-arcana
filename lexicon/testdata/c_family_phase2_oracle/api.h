#ifndef PHASE2_API_H
#define PHASE2_API_H

typedef int (*binary_fn)(int left, int right);

int add(int left, int right);
int apply(binary_fn fn, int value);

#define APPLY_TWICE(fn, value) fn(value, value)

#endif
