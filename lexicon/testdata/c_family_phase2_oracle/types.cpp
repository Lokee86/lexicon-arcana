#include "types.hpp"

namespace demo {

int Base::value(int input) const {
    return input;
}

int Derived::value(int input) const {
    return input + 1;
}

int choose(int value) {
    return value;
}

int choose(double value) {
    return static_cast<int>(value);
}

int invoke(Derived& target, int input) {
    int member = target.value(input);
    int overloaded = choose(input);
    return member + overloaded;
}

}
