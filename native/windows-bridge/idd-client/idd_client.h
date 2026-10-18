#pragma once

#include <stdint.h>

#ifdef _WIN32
#  define CW_IDD_API __declspec(dllexport)
#  define CW_IDD_CALL __stdcall
#else
#  define CW_IDD_API
#  define CW_IDD_CALL
#endif

typedef struct cw_idd_client cw_idd_client;

typedef struct cw_idd_frame_info {
    uint32_t width;
    uint32_t height;
    uint32_t stride;
    uint32_t pixel_format;
    uint64_t sequence;
    uint64_t captured_at_millis;
} cw_idd_frame_info;

#ifdef __cplusplus
extern "C" {
#endif

// Returns ERROR_SUCCESS or a Win32 error code. The handle is exclusive for the
// lifetime of the client and closing it revokes capture in the driver.
CW_IDD_API int32_t CW_IDD_CALL cw_idd_client_open(cw_idd_client** client_out);
CW_IDD_API void CW_IDD_CALL cw_idd_client_close(cw_idd_client* client);
CW_IDD_API int32_t CW_IDD_CALL cw_idd_client_set_capture(cw_idd_client* client, uint32_t enabled);

// `output` contains `cw_idd_frame_info` followed by tightly-packed BGRA pixels.
// Returns ERROR_SUCCESS, ERROR_NO_MORE_ITEMS for no newer frame, or a Win32 error.
CW_IDD_API int32_t CW_IDD_CALL cw_idd_client_read_frame(
    cw_idd_client* client,
    uint64_t after_sequence,
    void* output,
    uint32_t output_length,
    uint32_t* bytes_returned);

#ifdef __cplusplus
}
#endif
