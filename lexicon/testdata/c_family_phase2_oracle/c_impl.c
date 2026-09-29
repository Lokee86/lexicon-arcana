#include "api.h"

int add(int left, int right) {
    return left + right;
}

static int subtract(int left, int right) {
    return left - right;
}

int apply(binary_fn fn, int value) {
    return fn(value, 1);
}

int run_c(int value) {
    binary_fn selected = add;
    int direct = add(value, 2);
    int macro_value = APPLY_TWICE(add, value);
    int indirect = selected(value, 3);
    return direct + macro_value + indirect + subtract(value, 1);
}
