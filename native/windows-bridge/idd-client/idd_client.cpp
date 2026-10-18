#include "idd_client.h"

#include "../../idd/ioctl_protocol.h"

#include <Windows.h>
#include <SetupAPI.h>

#include <cstddef>
#include <memory>
#include <new>
#include <vector>

#pragma comment(lib, "Setupapi.lib")

struct cw_idd_client {
    HANDLE device{INVALID_HANDLE_VALUE};
};

namespace {
constexpr DWORD kMaxOutputBytes = sizeof(cw_idd_frame_info) + 3840u * 2160u * 4u;

int32_t open_device(HANDLE& device) noexcept {
    const HDEVINFO devices = SetupDiGetClassDevsW(
        &GUID_DEVINTERFACE_CROSSWARP_IDD, nullptr, nullptr,
        DIGCF_PRESENT | DIGCF_DEVICEINTERFACE);
    if (devices == INVALID_HANDLE_VALUE) return static_cast<int32_t>(GetLastError());

    int32_t result = ERROR_DEVICE_NOT_CONNECTED;
    for (DWORD index = 0;; ++index) {
        SP_DEVICE_INTERFACE_DATA interface_data{};
        interface_data.cbSize = sizeof(interface_data);
        if (!SetupDiEnumDeviceInterfaces(devices, nullptr, &GUID_DEVINTERFACE_CROSSWARP_IDD,
                index, &interface_data)) {
            const DWORD error = GetLastError();
            if (error != ERROR_NO_MORE_ITEMS) result = static_cast<int32_t>(error);
            break;
        }

        DWORD required = 0;
        SetupDiGetDeviceInterfaceDetailW(devices, &interface_data, nullptr, 0, &required, nullptr);
        if (required < sizeof(SP_DEVICE_INTERFACE_DETAIL_DATA_W)) {
            result = static_cast<int32_t>(GetLastError());
            continue;
        }

        std::vector<std::byte> storage;
        try {
            storage.resize(required);
        } catch (...) {
            result = ERROR_NOT_ENOUGH_MEMORY;
            break;
        }
        auto* detail = reinterpret_cast<SP_DEVICE_INTERFACE_DETAIL_DATA_W*>(storage.data());
        detail->cbSize = sizeof(SP_DEVICE_INTERFACE_DETAIL_DATA_W);
        if (!SetupDiGetDeviceInterfaceDetailW(devices, &interface_data, detail, required, nullptr, nullptr)) {
            result = static_cast<int32_t>(GetLastError());
            continue;
        }

        device = CreateFileW(detail->DevicePath, GENERIC_READ | GENERIC_WRITE, 0, nullptr,
            OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
        if (device != INVALID_HANDLE_VALUE) {
            result = ERROR_SUCCESS;
            break;
        }
        result = static_cast<int32_t>(GetLastError());
    }

    SetupDiDestroyDeviceInfoList(devices);
    return result;
}
} // namespace

extern "C" int32_t CW_IDD_CALL cw_idd_client_open(cw_idd_client** client_out) {
    if (!client_out) return ERROR_INVALID_PARAMETER;
    *client_out = nullptr;
    auto client = std::unique_ptr<cw_idd_client>(new (std::nothrow) cw_idd_client{});
    if (!client) return ERROR_NOT_ENOUGH_MEMORY;
    const int32_t status = open_device(client->device);
    if (status != ERROR_SUCCESS) return status;
    *client_out = client.release();
    return ERROR_SUCCESS;
}

extern "C" void CW_IDD_CALL cw_idd_client_close(cw_idd_client* client) {
    if (!client) return;
    if (client->device != INVALID_HANDLE_VALUE) {
        ULONG disable = 0;
        DWORD returned = 0;
        DeviceIoControl(client->device, IOCTL_CROSSWARP_IDD_SET_CAPTURE,
            &disable, sizeof(disable), nullptr, 0, &returned, nullptr);
        CloseHandle(client->device);
    }
    delete client;
}

extern "C" int32_t CW_IDD_CALL cw_idd_client_set_capture(cw_idd_client* client, uint32_t enabled) {
    if (!client || enabled > 1) return ERROR_INVALID_PARAMETER;
    if (client->device == INVALID_HANDLE_VALUE) return ERROR_INVALID_HANDLE;
    ULONG value = enabled;
    DWORD returned = 0;
    if (DeviceIoControl(client->device, IOCTL_CROSSWARP_IDD_SET_CAPTURE,
            &value, sizeof(value), nullptr, 0, &returned, nullptr)) {
        return ERROR_SUCCESS;
    }
    return static_cast<int32_t>(GetLastError());
}

extern "C" int32_t CW_IDD_CALL cw_idd_client_read_frame(cw_idd_client* client,
    uint64_t after_sequence, void* output, uint32_t output_length, uint32_t* bytes_returned) {
    if (bytes_returned) *bytes_returned = 0;
    if (!client || !output || !bytes_returned ||
        output_length < sizeof(cw_idd_frame_info) || output_length > kMaxOutputBytes) {
        return ERROR_INVALID_PARAMETER;
    }
    if (client->device == INVALID_HANDLE_VALUE) return ERROR_INVALID_HANDLE;

    CrossWarpIddFrameRequest request{after_sequence};
    DWORD returned = 0;
    if (!DeviceIoControl(client->device, IOCTL_CROSSWARP_IDD_GET_FRAME,
            &request, sizeof(request),
            output, output_length, &returned, nullptr)) {
        *bytes_returned = returned;
        return static_cast<int32_t>(GetLastError());
    }
    if (returned < sizeof(cw_idd_frame_info)) return ERROR_INVALID_DATA;
    *bytes_returned = returned;
    return ERROR_SUCCESS;
}
