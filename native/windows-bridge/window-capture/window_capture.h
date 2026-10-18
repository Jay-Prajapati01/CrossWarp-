#pragma once
#include <stddef.h>
#include <stdint.h>
#ifdef _WIN32
#define CW_CAPTURE_API __declspec(dllexport)
#define CW_CAPTURE_CALL __cdecl
#else
#define CW_CAPTURE_API
#define CW_CAPTURE_CALL
#endif
#ifdef __cplusplus
extern "C" {
#endif
typedef void (CW_CAPTURE_CALL *cw_window_callback)(
    uint64_t hwnd, const uint16_t* title, size_t title_length, void* context);
CW_CAPTURE_API void CW_CAPTURE_CALL cw_capture_enumerate_windows(
    cw_window_callback callback, void* context);
CW_CAPTURE_API int32_t CW_CAPTURE_CALL cw_capture_start(uint64_t hwnd, void** capture_out);
CW_CAPTURE_API int32_t CW_CAPTURE_CALL cw_capture_read(
    void* capture, uint64_t after_sequence, uint8_t* buffer, size_t capacity,
    size_t* required, uint32_t* width, uint32_t* height, uint64_t* sequence);
CW_CAPTURE_API const char* CW_CAPTURE_CALL cw_capture_last_error(void);
CW_CAPTURE_API void CW_CAPTURE_CALL cw_capture_stop(void* capture);
#ifdef __cplusplus
}
#endif
