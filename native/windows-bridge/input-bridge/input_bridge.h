#pragma once

#include <stdint.h>

#ifdef _WIN32
#  if defined(UNIFIED_INPUT_BRIDGE_EXPORTS)
#    define UW_INPUT_API __declspec(dllexport)
#  elif !defined(UNIFIED_INPUT_BRIDGE_STATIC)
#    define UW_INPUT_API __declspec(dllimport)
#  else
#    define UW_INPUT_API
#  endif
#  define UW_INPUT_CALL __stdcall
#else
#  define UW_INPUT_API
#  define UW_INPUT_CALL
#endif

typedef struct uw_input_capture uw_input_capture;

typedef enum uw_input_kind {
    UW_INPUT_POINTER_MOVE = 1,
    UW_INPUT_POINTER_BUTTON = 2,
    UW_INPUT_POINTER_WHEEL = 3,
    UW_INPUT_KEY = 4,
} uw_input_kind;

enum {
    UW_INPUT_FLAG_ABSOLUTE = 1u << 0,
    UW_INPUT_FLAG_HORIZONTAL = 1u << 1,
    UW_INPUT_FLAG_EXTENDED_KEY = 1u << 2,
    UW_INPUT_FLAG_PREVIOUS_KEY_STATE = 1u << 3,
    UW_INPUT_FLAG_E1_KEY = 1u << 4,
};

typedef struct uw_input_event {
    uint32_t kind;
    uint32_t code;       /* button number 1..5 or virtual-key code */
    int32_t x;           /* pointer delta, or normalized absolute x (0..65535) */
    int32_t y;           /* pointer delta, or normalized absolute y (0..65535) */
    int32_t value;       /* button/key: 1 down, 0 up; wheel: signed delta */
    uint32_t flags;
    uint32_t scan_code;
    uint32_t message_time_ms;
} uw_input_event;

typedef void (UW_INPUT_CALL *uw_input_event_callback)(const uw_input_event* event, void* context);

#ifdef __cplusplus
extern "C" {
#endif

/* Starts a message-only Raw Input sink for mouse and keyboard in this user session.
   It does not suppress legacy input. Caller must obtain explicit user consent first. */
UW_INPUT_API int32_t UW_INPUT_CALL uw_input_capture_start(
    uw_input_event_callback callback,
    void* context,
    uw_input_capture** capture_out);

/* Stops capture, unregisters both devices, waits for the message thread, and frees the handle. */
UW_INPUT_API void UW_INPUT_CALL uw_input_capture_stop(uw_input_capture* capture);

/* Injects one mouse or keyboard transition using SendInput. Returns 1 on insertion,
   0 when Windows rejected/blocked it, or a negative value for invalid input. UIPI
   restrictions are not bypassed; GetLastError may not identify UIPI as the cause. */
UW_INPUT_API int32_t UW_INPUT_CALL uw_input_inject(const uw_input_event* event);

#ifdef __cplusplus
}
#endif
