#pragma once

#include <Windows.h>
#include <winioctl.h>
#include <cstdint>

// Local user-session/IDD bridge only. These IOCTLs never cross the network;
// the normal CrossWarp session remains responsible for peer authentication.
constexpr GUID GUID_DEVINTERFACE_CROSSWARP_IDD = {
    0x8e4a7961, 0xa29d, 0x4f67, {0x9f, 0x5e, 0x6c, 0x30, 0x9b, 0x57, 0x25, 0x21}
};

constexpr ULONG IOCTL_CROSSWARP_IDD_SET_CAPTURE =
    CTL_CODE(FILE_DEVICE_UNKNOWN, 0x800, METHOD_BUFFERED, FILE_WRITE_ACCESS);
constexpr ULONG IOCTL_CROSSWARP_IDD_GET_FRAME =
    CTL_CODE(FILE_DEVICE_UNKNOWN, 0x801, METHOD_OUT_DIRECT, FILE_READ_ACCESS);

struct CrossWarpIddFrameRequest {
    std::uint64_t after_sequence;
};

static_assert(sizeof(CrossWarpIddFrameRequest) == sizeof(std::uint64_t),
    "IDD frame IOCTL request layout must remain stable");
