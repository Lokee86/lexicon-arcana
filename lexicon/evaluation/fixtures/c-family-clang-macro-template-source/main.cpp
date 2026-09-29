int target(int value) { return value; }

#define MAKE_HELPER(name)                                                   \
  template <typename T, typename U>                                         \
  int CmpHelper##name(const char*, const char*, const T& a, const U&) {      \
    return target(a);                                                        \
  }

MAKE_HELPER(NE)

int use() { return CmpHelperNE("", "", 1, 2); }
