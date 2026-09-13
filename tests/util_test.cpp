#include "../src/util.h"
#include <cassert>
#include <iostream>
int main() {
    assert(timestamp("90") == 90000);
    assert(timestamp("1:02.5") == 62500);
    assert(timestamp("1:02:03") == 3723000);
    assert(timestamp("0") == 0);
    for (const char *bad : {"", "-1", "nan", "inf", "1:60", "1.5:00", "1::2", "1:2:3:4", "10000000000000"})
        assert(!timestamp(bad));
    std::cout << "Timestamp parsing passed\n";
}
