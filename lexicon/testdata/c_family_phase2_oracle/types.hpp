#ifndef PHASE2_TYPES_HPP
#define PHASE2_TYPES_HPP

namespace demo {

struct Base {
    virtual int value(int input) const;
};

struct Derived : Base {
    int value(int input) const;
};

int choose(int value);
int choose(double value);
int invoke(Derived& target, int input);

}

#endif
