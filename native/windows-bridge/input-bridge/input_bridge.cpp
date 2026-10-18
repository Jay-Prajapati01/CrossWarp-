#include "input_bridge.h"

#include <Windows.h>
#include <hidusage.h>

#include <atomic>
#include <cstddef>
#include <new>
#include <vector>

struct uw_input_capture {
    HANDLE thread{};
    HANDLE ready{};
    DWORD thread_id{};
    HWND window{};
    uw_input_event_callback callback{};
    void* context{};
    std::atomic<LONG> startup_error{ERROR_SUCCESS};
};

namespace {
constexpr wchar_t kWindowClass[] = L"UnifiedWorkspace.RawInputSink";

uw_input_event with_cursor(uw_input_event event) noexcept {
    POINT point{};
    if (GetCursorPos(&point)) {
        event.cursor_x = point.x;
        event.cursor_y = point.y;
        event.flags |= UW_INPUT_FLAG_CURSOR_POSITION_VALID;
    }
    return event;
}

void publish(uw_input_capture* capture, const uw_input_event& event) noexcept {
    if (capture && capture->callback) {
        try {
            capture->callback(&event, capture->context);
        } catch (...) {
            // C callbacks must not unwind through the Windows message loop.
        }
    }
}

void handle_raw_input(uw_input_capture* capture, HRAWINPUT raw) {
    UINT bytes = 0;
    if (GetRawInputData(raw, RID_INPUT, nullptr, &bytes, sizeof(RAWINPUTHEADER)) != 0 || bytes == 0) return;
    std::vector<std::byte> storage(bytes);
    if (GetRawInputData(raw, RID_INPUT, storage.data(), &bytes, sizeof(RAWINPUTHEADER)) != bytes) return;
    const auto* input = reinterpret_cast<const RAWINPUT*>(storage.data());
    const auto time = static_cast<uint32_t>(GetMessageTime());

    if (input->header.dwType == RIM_TYPEMOUSE) {
        const auto& mouse = input->data.mouse;
        if (mouse.lLastX != 0 || mouse.lLastY != 0 || (mouse.usFlags & MOUSE_MOVE_ABSOLUTE)) {
            publish(capture, with_cursor({UW_INPUT_POINTER_MOVE, 0, mouse.lLastX, mouse.lLastY, 0,
                (mouse.usFlags & MOUSE_MOVE_ABSOLUTE) ? UW_INPUT_FLAG_ABSOLUTE : 0u, 0, time}));
        }
        const USHORT flags = mouse.usButtonFlags;
        const auto button = [&](USHORT down_flag, USHORT up_flag, uint32_t number) {
            if (flags & down_flag) publish(capture, with_cursor({UW_INPUT_POINTER_BUTTON, number, 0, 0, 1, 0, 0, time}));
            if (flags & up_flag) publish(capture, with_cursor({UW_INPUT_POINTER_BUTTON, number, 0, 0, 0, 0, 0, time}));
        };
        button(RI_MOUSE_BUTTON_1_DOWN, RI_MOUSE_BUTTON_1_UP, 1);
        button(RI_MOUSE_BUTTON_2_DOWN, RI_MOUSE_BUTTON_2_UP, 2);
        button(RI_MOUSE_BUTTON_3_DOWN, RI_MOUSE_BUTTON_3_UP, 3);
        button(RI_MOUSE_BUTTON_4_DOWN, RI_MOUSE_BUTTON_4_UP, 4);
        button(RI_MOUSE_BUTTON_5_DOWN, RI_MOUSE_BUTTON_5_UP, 5);
        if (flags & (RI_MOUSE_WHEEL | RI_MOUSE_HWHEEL)) {
            publish(capture, with_cursor({UW_INPUT_POINTER_WHEEL, 0, 0, 0,
                static_cast<SHORT>(mouse.usButtonData),
                (flags & RI_MOUSE_HWHEEL) ? UW_INPUT_FLAG_HORIZONTAL : 0u, 0, time}));
        }
        return;
    }

    if (input->header.dwType == RIM_TYPEKEYBOARD) {
        const auto& keyboard = input->data.keyboard;
        uint32_t flags = 0;
        if (keyboard.Flags & RI_KEY_E0) flags |= UW_INPUT_FLAG_EXTENDED_KEY;
        if (keyboard.Flags & RI_KEY_E1) flags |= UW_INPUT_FLAG_E1_KEY;
        const bool key_up = (keyboard.Flags & RI_KEY_BREAK) != 0;
        publish(capture, with_cursor({UW_INPUT_KEY, keyboard.VKey, 0, 0, key_up ? 0 : 1, flags,
            keyboard.MakeCode, time}));
    }
}

LRESULT CALLBACK sink_window_proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    auto* capture = reinterpret_cast<uw_input_capture*>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) {
        const auto* create = reinterpret_cast<const CREATESTRUCTW*>(lparam);
        capture = static_cast<uw_input_capture*>(create->lpCreateParams);
        SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(capture));
    }
    if (message == WM_INPUT && capture) {
        handle_raw_input(capture, reinterpret_cast<HRAWINPUT>(lparam));
        return DefWindowProcW(window, message, wparam, lparam);
    }
    return DefWindowProcW(window, message, wparam, lparam);
}

DWORD WINAPI capture_thread(void* parameter) {
    auto* capture = static_cast<uw_input_capture*>(parameter);
    WNDCLASSEXW wc{};
    wc.cbSize = sizeof(wc);
    wc.lpfnWndProc = sink_window_proc;
    wc.hInstance = GetModuleHandleW(nullptr);
    wc.lpszClassName = kWindowClass;
    if (!RegisterClassExW(&wc) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) {
        capture->startup_error.store(static_cast<LONG>(GetLastError()));
        SetEvent(capture->ready);
        return 1;
    }

    capture->window = CreateWindowExW(0, kWindowClass, L"", 0, 0, 0, 0, 0,
        HWND_MESSAGE, nullptr, wc.hInstance, capture);
    if (!capture->window) {
        capture->startup_error.store(static_cast<LONG>(GetLastError()));
        SetEvent(capture->ready);
        return 1;
    }

    RAWINPUTDEVICE devices[2]{};
    devices[0] = {HID_USAGE_PAGE_GENERIC, HID_USAGE_GENERIC_MOUSE, RIDEV_INPUTSINK, capture->window};
    devices[1] = {HID_USAGE_PAGE_GENERIC, HID_USAGE_GENERIC_KEYBOARD, RIDEV_INPUTSINK, capture->window};
    if (!RegisterRawInputDevices(devices, 2, sizeof(RAWINPUTDEVICE))) {
        capture->startup_error.store(static_cast<LONG>(GetLastError()));
        DestroyWindow(capture->window);
        capture->window = nullptr;
        SetEvent(capture->ready);
        return 1;
    }
    // A caller can stop capture immediately after startup returns. Ensure the
    // thread's message queue exists before signaling ready, otherwise its
    // PostThreadMessage(WM_QUIT) could fail in the startup race and stop would
    // wait forever for a message loop that never receives its shutdown request.
    MSG bootstrap{};
    PeekMessageW(&bootstrap, nullptr, WM_USER, WM_USER, PM_NOREMOVE);
    SetEvent(capture->ready);

    MSG message{};
    while (GetMessageW(&message, nullptr, 0, 0) > 0) {
        TranslateMessage(&message);
        DispatchMessageW(&message);
    }

    RAWINPUTDEVICE remove[2]{};
    remove[0] = {HID_USAGE_PAGE_GENERIC, HID_USAGE_GENERIC_MOUSE, RIDEV_REMOVE, nullptr};
    remove[1] = {HID_USAGE_PAGE_GENERIC, HID_USAGE_GENERIC_KEYBOARD, RIDEV_REMOVE, nullptr};
    RegisterRawInputDevices(remove, 2, sizeof(RAWINPUTDEVICE));
    if (capture->window) DestroyWindow(capture->window);
    UnregisterClassW(kWindowClass, wc.hInstance);
    return 0;
}

bool build_input(const uw_input_event& event, INPUT& input) {
    input = {};
    input.type = event.kind == UW_INPUT_KEY ? INPUT_KEYBOARD : INPUT_MOUSE;
    if (event.kind == UW_INPUT_POINTER_MOVE) {
        input.type = INPUT_MOUSE;
        input.mi.dx = event.x;
        input.mi.dy = event.y;
        input.mi.dwFlags = MOUSEEVENTF_MOVE;
        if (event.flags & UW_INPUT_FLAG_ABSOLUTE) {
            input.mi.dwFlags |= MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK;
        }
        return true;
    }
    if (event.kind == UW_INPUT_POINTER_BUTTON) {
        input.type = INPUT_MOUSE;
        switch (event.code) {
        case 1: input.mi.dwFlags = event.value ? MOUSEEVENTF_LEFTDOWN : MOUSEEVENTF_LEFTUP; break;
        case 2: input.mi.dwFlags = event.value ? MOUSEEVENTF_RIGHTDOWN : MOUSEEVENTF_RIGHTUP; break;
        case 3: input.mi.dwFlags = event.value ? MOUSEEVENTF_MIDDLEDOWN : MOUSEEVENTF_MIDDLEUP; break;
        case 4:
        case 5:
            input.mi.dwFlags = event.value ? MOUSEEVENTF_XDOWN : MOUSEEVENTF_XUP;
            input.mi.mouseData = event.code == 4 ? XBUTTON1 : XBUTTON2;
            break;
        default: return false;
        }
        return true;
    }
    if (event.kind == UW_INPUT_POINTER_WHEEL) {
        input.type = INPUT_MOUSE;
        input.mi.mouseData = static_cast<DWORD>(event.value);
        input.mi.dwFlags = (event.flags & UW_INPUT_FLAG_HORIZONTAL) ? MOUSEEVENTF_HWHEEL : MOUSEEVENTF_WHEEL;
        return true;
    }
    if (event.kind == UW_INPUT_KEY) {
        input.type = INPUT_KEYBOARD;
        input.ki.wVk = static_cast<WORD>(event.code);
        input.ki.wScan = static_cast<WORD>(event.scan_code);
        if (!event.value) input.ki.dwFlags |= KEYEVENTF_KEYUP;
        if (event.flags & UW_INPUT_FLAG_EXTENDED_KEY) input.ki.dwFlags |= KEYEVENTF_EXTENDEDKEY;
        return event.code <= 0xFF;
    }
    return false;
}
} // namespace

extern "C" int32_t UW_INPUT_CALL uw_input_capture_start(
    uw_input_event_callback callback,
    void* context,
    uw_input_capture** capture_out) {
    if (!callback || !capture_out) return ERROR_INVALID_PARAMETER;
    *capture_out = nullptr;
    auto* capture = new (std::nothrow) uw_input_capture{};
    if (!capture) return ERROR_NOT_ENOUGH_MEMORY;
    capture->callback = callback;
    capture->context = context;
    capture->ready = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    if (!capture->ready) {
        const auto error = GetLastError();
        delete capture;
        return static_cast<int32_t>(error);
    }
    capture->thread = CreateThread(nullptr, 0, capture_thread, capture, 0, &capture->thread_id);
    if (!capture->thread) {
        const auto error = GetLastError();
        CloseHandle(capture->ready);
        delete capture;
        return static_cast<int32_t>(error);
    }
    WaitForSingleObject(capture->ready, INFINITE);
    const auto error = capture->startup_error.load();
    if (error != ERROR_SUCCESS) {
        WaitForSingleObject(capture->thread, INFINITE);
        CloseHandle(capture->thread);
        CloseHandle(capture->ready);
        delete capture;
        return error;
    }
    *capture_out = capture;
    return ERROR_SUCCESS;
}

extern "C" void UW_INPUT_CALL uw_input_capture_stop(uw_input_capture* capture) {
    if (!capture) return;
    PostThreadMessageW(capture->thread_id, WM_QUIT, 0, 0);
    WaitForSingleObject(capture->thread, INFINITE);
    CloseHandle(capture->thread);
    CloseHandle(capture->ready);
    delete capture;
}

extern "C" int32_t UW_INPUT_CALL uw_input_inject(const uw_input_event* event) {
    if (!event) return -1;
    INPUT input{};
    if (!build_input(*event, input)) return -1;
    SetLastError(ERROR_SUCCESS);
    const UINT inserted = SendInput(1, &input, sizeof(INPUT));
    if (inserted == 1) return 1;
    return 0;
}
