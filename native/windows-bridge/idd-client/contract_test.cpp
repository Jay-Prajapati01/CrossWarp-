#include "idd_client.h"

#include <Windows.h>

#include <cassert>
#include <cstdint>
#include <iostream>

int wmain() {
    assert(cw_idd_client_open(nullptr) == ERROR_INVALID_PARAMETER);
    assert(cw_idd_client_set_capture(nullptr, 1) == ERROR_INVALID_PARAMETER);
    assert(cw_idd_client_read_frame(nullptr, 0, nullptr, 0, nullptr) == ERROR_INVALID_PARAMETER);

    cw_idd_frame_info info{};
    uint32_t returned = 0;
    assert(cw_idd_client_set_capture(nullptr, 2) == ERROR_INVALID_PARAMETER);
    assert(cw_idd_client_read_frame(nullptr, 0, &info, sizeof(info) - 1, &returned) == ERROR_INVALID_PARAMETER);
    std::wcout << L"IDD client argument contract passed; no device opened or capture enabled\n";
    return 0;
}
