#include "input_bridge.h"

#include <Windows.h>

#include <cassert>
#include <cstdint>
#include <iostream>

int wmain() {
    const uw_input_event invalid_button{UW_INPUT_POINTER_BUTTON, 9, 0, 0, 1, 0, 0, 0};
    assert(uw_input_inject(&invalid_button) == -1);
    const uw_input_event invalid_kind{99, 0, 0, 0, 0, 0, 0, 0};
    assert(uw_input_inject(&invalid_kind) == -1);
    assert(uw_input_inject(nullptr) == -1);
    std::wcout << L"input bridge argument contract passed; no system input was injected\n";
    return 0;
}
