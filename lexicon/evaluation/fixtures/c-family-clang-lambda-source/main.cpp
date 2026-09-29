int target(int value) { return value + 1; }

template <typename T>
int outer(T value) {
  auto invoke = [&]() { return target(value); };
  return invoke();
}

int use() { return outer(41); }
