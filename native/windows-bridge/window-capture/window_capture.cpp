#include "window_capture.h"
#include <Windows.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <windows.graphics.capture.interop.h>
#include <windows.graphics.directx.direct3d11.interop.h>
#include <winrt/Windows.Graphics.Capture.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Graphics.DirectX.h>
#include <winrt/Windows.Graphics.DirectX.Direct3D11.h>
#include <winrt/base.h>
#include <cstring>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <string>
#include <vector>

using namespace winrt;
using winrt::Windows::Graphics::Capture::Direct3D11CaptureFramePool;
using winrt::Windows::Graphics::Capture::GraphicsCaptureItem;
using winrt::Windows::Graphics::Capture::GraphicsCaptureSession;
using winrt::Windows::Graphics::DirectX::DirectXPixelFormat;
using winrt::Windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;

namespace {
thread_local std::string last_error;
constexpr uint64_t maximum_frame_bytes = 64ull * 1024ull * 1024ull;

struct ApartmentGuard {
    ApartmentGuard() { init_apartment(apartment_type::multi_threaded); }
    ~ApartmentGuard() { uninit_apartment(); }
};

struct EnumerateContext {
    cw_window_callback callback;
    void* user;
};

BOOL CALLBACK enumerate_window(HWND window, LPARAM raw) {
    auto* context = reinterpret_cast<EnumerateContext*>(raw);
    if (!IsWindowVisible(window)) return TRUE;
    const int length = GetWindowTextLengthW(window);
    if (length <= 0 || length > 4096) return TRUE;
    std::vector<wchar_t> title(static_cast<size_t>(length) + 1);
    const int copied = GetWindowTextW(window, title.data(), static_cast<int>(title.size()));
    if (copied <= 0) return TRUE;
    DWORD owner = 0;
    GetWindowThreadProcessId(window, &owner);
    if (owner == GetCurrentProcessId()) return TRUE;
    context->callback(static_cast<uint64_t>(reinterpret_cast<uintptr_t>(window)),
        reinterpret_cast<const uint16_t*>(title.data()), static_cast<size_t>(copied), context->user);
    return TRUE;
}

IDirect3DDevice to_winrt_device(ID3D11Device* device) {
    com_ptr<IDXGIDevice> dxgi;
    check_hresult(device->QueryInterface(IID_PPV_ARGS(dxgi.put())));
    com_ptr<IInspectable> inspectable;
    check_hresult(CreateDirect3D11DeviceFromDXGIDevice(dxgi.get(), inspectable.put()));
    return inspectable.as<IDirect3DDevice>();
}

GraphicsCaptureItem item_for_window(HWND window) {
    auto factory = get_activation_factory<GraphicsCaptureItem, IGraphicsCaptureItemInterop>();
    GraphicsCaptureItem item{nullptr};
    check_hresult(factory->CreateForWindow(window, guid_of<GraphicsCaptureItem>(), put_abi(item)));
    return item;
}

struct CaptureState {
    ApartmentGuard apartment;
    explicit CaptureState(HWND target) {
        window = target;
        if (!IsWindow(target) || !IsWindowVisible(target)) throw std::runtime_error("selected window is not visible");
        if (!GraphicsCaptureSession::IsSupported()) throw std::runtime_error("Windows.Graphics.Capture is not supported on this build");
        D3D_FEATURE_LEVEL level{};
        com_ptr<ID3D11DeviceContext> context;
        auto status = D3D11CreateDevice(nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, nullptr, 0, D3D11_SDK_VERSION,
            device.put(), &level, context.put());
        if (FAILED(status)) {
            status = D3D11CreateDevice(nullptr, D3D_DRIVER_TYPE_WARP, nullptr,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, nullptr, 0, D3D11_SDK_VERSION,
                device.put(), &level, context.put());
        }
        check_hresult(status);
        immediate = context;
        item = item_for_window(target);
        const auto size = item.Size();
        if (size.Width <= 0 || size.Height <= 0 || size.Width > 16384 || size.Height > 16384)
            throw std::runtime_error("capture item has invalid dimensions");
        pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            to_winrt_device(device.get()), DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, size);
        session = pool.CreateCaptureSession(item);
        token = pool.FrameArrived([this](const Direct3D11CaptureFramePool& sender,
                                         const winrt::Windows::Foundation::IInspectable&) {
            receive_frame(sender);
        });
        session.StartCapture();
    }

    ~CaptureState() {
        try {
            session.Close();
            pool.FrameArrived(token);
            pool.Close();
        } catch (...) {}
    }

    void receive_frame(const Direct3D11CaptureFramePool& sender) noexcept {
        try {
            auto frame = sender.TryGetNextFrame();
            if (!frame) return;
            auto access = frame.Surface().as<::Windows::Graphics::DirectX::Direct3D11::IDirect3DDxgiInterfaceAccess>();
            com_ptr<ID3D11Texture2D> source;
            check_hresult(access->GetInterface(__uuidof(ID3D11Texture2D), source.put_void()));
            D3D11_TEXTURE2D_DESC desc{};
            source->GetDesc(&desc);
            const uint64_t bytes = static_cast<uint64_t>(desc.Width) * desc.Height * 4;
            if (!desc.Width || !desc.Height || bytes > maximum_frame_bytes ||
                desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM)
                throw std::runtime_error("capture frame format or dimensions are unsupported");
            D3D11_TEXTURE2D_DESC stagingDesc = desc;
            stagingDesc.MipLevels = 1;
            stagingDesc.ArraySize = 1;
            stagingDesc.SampleDesc.Count = 1;
            stagingDesc.SampleDesc.Quality = 0;
            stagingDesc.Usage = D3D11_USAGE_STAGING;
            stagingDesc.BindFlags = 0;
            stagingDesc.MiscFlags = 0;
            stagingDesc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
            com_ptr<ID3D11Texture2D> staging;
            check_hresult(device->CreateTexture2D(&stagingDesc, nullptr, staging.put()));
            immediate->CopyResource(staging.get(), source.get());
            D3D11_MAPPED_SUBRESOURCE mapped{};
            check_hresult(immediate->Map(staging.get(), 0, D3D11_MAP_READ, 0, &mapped));
            std::vector<uint8_t> pixels(static_cast<size_t>(bytes));
            for (uint32_t y = 0; y < desc.Height; ++y)
                std::memcpy(pixels.data() + static_cast<size_t>(y) * desc.Width * 4,
                    static_cast<const uint8_t*>(mapped.pData) + static_cast<size_t>(y) * mapped.RowPitch,
                    static_cast<size_t>(desc.Width) * 4);
            immediate->Unmap(staging.get(), 0);
            std::lock_guard lock(mutex);
            latest = std::move(pixels);
            width = desc.Width;
            height = desc.Height;
            ++sequence;
            error.clear();
        } catch (const hresult_error& e) {
            std::lock_guard lock(mutex);
            error = winrt::to_string(e.message());
        } catch (const std::exception& e) {
            std::lock_guard lock(mutex);
            error = e.what();
        } catch (...) {
            std::lock_guard lock(mutex);
            error = "unknown Windows.Graphics.Capture error";
        }
    }

    com_ptr<ID3D11Device> device;
    com_ptr<ID3D11DeviceContext> immediate;
    GraphicsCaptureItem item{nullptr};
    Direct3D11CaptureFramePool pool{nullptr};
    GraphicsCaptureSession session{nullptr};
    winrt::event_token token{};
    std::mutex mutex;
    std::vector<uint8_t> latest;
    std::string error;
    uint32_t width{};
    uint32_t height{};
    uint64_t sequence{};
    HWND window{};
};
}

extern "C" void CW_CAPTURE_CALL cw_capture_enumerate_windows(
    cw_window_callback callback, void* user) {
    if (!callback) return;
    EnumerateContext context{callback, user};
    EnumWindows(enumerate_window, reinterpret_cast<LPARAM>(&context));
}

extern "C" int32_t CW_CAPTURE_CALL cw_capture_start(uint64_t raw, void** output) {
    if (!output) return -1;
    *output = nullptr;
    try {
        auto capture = std::make_unique<CaptureState>(reinterpret_cast<HWND>(static_cast<uintptr_t>(raw)));
        *output = capture.release();
        last_error.clear();
        return 0;
    } catch (const hresult_error& e) {
        last_error = winrt::to_string(e.message());
        return static_cast<int32_t>(e.code());
    } catch (const std::exception& e) {
        last_error = e.what();
        return -1;
    } catch (...) {
        last_error = "unknown Windows.Graphics.Capture startup error";
        return -1;
    }
}

extern "C" int32_t CW_CAPTURE_CALL cw_capture_read(
    void* raw, uint64_t after, uint8_t* buffer, size_t capacity, size_t* required,
    uint32_t* width, uint32_t* height, uint64_t* sequence) {
    if (!raw || !required || !width || !height || !sequence) return -1;
    auto* state = static_cast<CaptureState*>(raw);
    std::lock_guard lock(state->mutex);
    if (!IsWindow(state->window)) {
        state->error = "selected window closed during capture";
    }
    if (!state->error.empty()) {
        last_error = state->error;
        return -1;
    }
    if (state->sequence <= after || state->latest.empty()) return 0;
    *required = state->latest.size();
    *width = state->width;
    *height = state->height;
    *sequence = state->sequence;
    if (capacity < state->latest.size() || !buffer) return -2;
    std::memcpy(buffer, state->latest.data(), state->latest.size());
    return 1;
}

extern "C" const char* CW_CAPTURE_CALL cw_capture_last_error(void) { return last_error.c_str(); }
extern "C" void CW_CAPTURE_CALL cw_capture_stop(void* capture) { delete static_cast<CaptureState*>(capture); }
