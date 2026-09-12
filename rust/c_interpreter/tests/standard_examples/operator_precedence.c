#include <assert.h>

int main(void) {
    /* Adjacent precedence levels and left associativity. */
    assert((2 + 3 * 4) == 14);
    assert((20 / 5 / 2) == 2);
    assert((20 - 5 - 2) == 13);
    assert((19 % 5 * 2) == 8);
    assert((1 << 2 + 1) == 8);
    assert((16 >> 1 >> 1) == 4);
    assert((2 < 1 << 2) == 1);
    assert((2 < 3 == 1) == 1);
    assert((2 & 3 == 3) == 0);
    assert((1 ^ 3 & 2) == 3);
    assert((1 | 3 ^ 1) == 3);
    assert((1 && 2 | 0) == 1);
    assert((1 || 0 && 0) == 1);
    assert((0 ? 2 : 1 ? 3 : 4) == 3);
    assert((1 ? 2, 3 : 4) == 3);
    int a = 0, b = 0;
    a = b = 5;
    assert(a == 5 && b == 5);
    a += b *= 2;
    assert(a == 15 && b == 10);
    /* Comma remains outside assignment and inside the middle ?: operand. */
    assert((a = 1, b = 2, a + b) == 3);
    assert((0 && ++a) == 0);
    assert((1 || ++b) == 1);
    assert(a == 1 && b == 2);
    return 0;
}
