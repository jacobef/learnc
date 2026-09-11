#include <string.h>
#include <wchar.h>

int main(void) {
    return strxfrm(NULL, "hello", 0) != 5 || wcsxfrm(NULL, L"hello", 0) != 5;
}
