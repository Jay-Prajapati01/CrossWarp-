#include <Windows.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <windows.graphics.capture.interop.h>
#include <windows.graphics.directx.direct3d11.interop.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Graphics.Capture.h>
#include <winrt/Windows.Graphics.DirectX.h>
#include <winrt/Windows.Graphics.DirectX.Direct3D11.h>
#include <winrt/base.h>

#include <condition_variable>
#include <chrono>
#include <cstring>
#include <cstdint>
#include <fstream>
#include <iostream>
#include <mutex>
#include <stdexcept>
#include <string>
#include <vector>

using namespace winrt;
using winrt::Windows::Graphics::Capture::Direct3D11CaptureFrame;
using winrt::Windows::Graphics::Capture::Direct3D11CaptureFramePool;
using winrt::Windows::Graphics::Capture::GraphicsCaptureItem;
using winrt::Windows::Graphics::Capture::GraphicsCaptureSession;
using winrt::Windows::Graphics::DirectX::DirectXPixelFormat;
using winrt::Windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;

#pragma pack(push, 1)
struct BitmapFileHeader {
    uint16_t type{0x4D42};
    uint32_t size{};
    uint16_t reserved1{};
    uint16_t reserved2{};
    uint32_t pixelOffset{54};
};
struct BitmapInfoHeader {
    uint32_t size{40};
    int32_t width{};
    int32_t height{};
    uint16_t planes{1};
    uint16_t bitsPerPixel{24};
    uint32_t compression{};
    uint32_t imageSize{};
    int32_t xPixelsPerMeter{};
    int32_t yPixelsPerMeter{};
    uint32_t colorsUsed{};
    uint32_t colorsImportant{};
};
#pragma pack(pop)

static_assert(sizeof(BitmapFileHeader) == 14);
static_assert(sizeof(BitmapInfoHeader) == 40);

static constexpr uint64_t MAX_RETAINED_FRAME_BYTES = 256ull * 1024ull * 1024ull;

static com_ptr<ID3D11Device> make_d3d_device() {
    com_ptr<ID3D11Device> device;
    com_ptr<ID3D11DeviceContext> context;
    D3D_FEATURE_LEVEL obtained{};
    check_hresult(D3D11CreateDevice(
        nullptr,
        D3D_DRIVER_TYPE_HARDWARE,
        nullptr,
        D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        nullptr,
        0,
        D3D11_SDK_VERSION,
        device.put(),
        &obtained,
        context.put()));
    return device;
}

static IDirect3DDevice to_winrt_device(ID3D11Device* device) {
    com_ptr<IDXGIDevice> dxgiDevice;
    check_hresult(device->QueryInterface(IID_PPV_ARGS(dxgiDevice.put())));
    com_ptr<IInspectable> inspectable;
    check_hresult(CreateDirect3D11DeviceFromDXGIDevice(dxgiDevice.get(), inspectable.put()));
    return inspectable.as<IDirect3DDevice>();
}

static GraphicsCaptureItem item_for_window(HWND window) {
    auto factory = get_activation_factory<GraphicsCaptureItem, IGraphicsCaptureItemInterop>();
    GraphicsCaptureItem item{nullptr};
    check_hresult(factory->CreateForWindow(window, guid_of<GraphicsCaptureItem>(), put_abi(item)));
    return item;
}

static void write_bmp(const std::wstring& path, int width, int height, const std::vector<uint8_t>& bgra) {
    const uint32_t rowBytes = static_cast<uint32_t>(width) * 3;
    const uint32_t paddedRowBytes = (rowBytes + 3u) & ~3u;
    std::vector<uint8_t> bgr(static_cast<size_t>(paddedRowBytes) * height, 0);
    for (int y = 0; y < height; ++y) {
        const auto* src = bgra.data() + static_cast<size_t>(y) * width * 4;
        auto* dst = bgr.data() + static_cast<size_t>(y) * paddedRowBytes;
        for (int x = 0; x < width; ++x) {
            dst[x * 3] = src[x * 4];
            dst[x * 3 + 1] = src[x * 4 + 1];
            dst[x * 3 + 2] = src[x * 4 + 2];
        }
    }

    BitmapFileHeader fileHeader;
    BitmapInfoHeader infoHeader;
    infoHeader.width = width;
    infoHeader.height = -height; // top-down rows
    infoHeader.imageSize = static_cast<uint32_t>(bgr.size());
    fileHeader.size = sizeof(fileHeader) + sizeof(infoHeader) + infoHeader.imageSize;

    std::ofstream file(path, std::ios::binary);
    if (!file) throw std::runtime_error("cannot create output bitmap");
    file.write(reinterpret_cast<const char*>(&fileHeader), sizeof(fileHeader));
    file.write(reinterpret_cast<const char*>(&infoHeader), sizeof(infoHeader));
    file.write(reinterpret_cast<const char*>(bgr.data()), static_cast<std::streamsize>(bgr.size()));
    if (!file) throw std::runtime_error("failed writing output bitmap");
}

int wmain(int argc, wchar_t** argv) {
    if (argc < 3 || argc > 4) {
        std::wcerr << L"Usage: window-capture-smoke.exe <HWND> <output.bmp|prefix> [frame-count 1..16]\n"
                   << L"Capture only a window you own or have explicit permission to capture.\n";
        return 2;
    }

    try {
        const auto rawWindow = std::wcstoull(argv[1], nullptr, 0);
        HWND window = reinterpret_cast<HWND>(static_cast<uintptr_t>(rawWindow));
        const int requestedFrames = argc == 4 ? std::stoi(argv[3]) : 1;
        if (requestedFrames < 1 || requestedFrames > 16) {
            throw std::runtime_error("frame-count must be between 1 and 16 to keep capture memory bounded");
        }
        if (!IsWindow(window) || !IsWindowVisible(window)) {
            throw std::runtime_error("HWND is not a visible window");
        }
        if (!GraphicsCaptureSession::IsSupported()) {
            throw std::runtime_error("Windows.Graphics.Capture is not supported on this OS build");
        }

        init_apartment(apartment_type::multi_threaded);
        auto d3dDevice = make_d3d_device();
        com_ptr<ID3D11DeviceContext> context;
        d3dDevice->GetImmediateContext(context.put());
        auto captureDevice = to_winrt_device(d3dDevice.get());
        auto item = item_for_window(window);
        const auto size = item.Size();
        if (size.Width <= 0 || size.Height <= 0 || size.Width > 16384 || size.Height > 16384) {
            throw std::runtime_error("capture item reports an invalid size");
        }
        const auto bytesPerFrame = static_cast<uint64_t>(size.Width) *
            static_cast<uint64_t>(size.Height) * 4ull;
        if (bytesPerFrame > MAX_RETAINED_FRAME_BYTES ||
            static_cast<uint64_t>(requestedFrames) > MAX_RETAINED_FRAME_BYTES / bytesPerFrame) {
            throw std::runtime_error("requested burst exceeds the 256 MiB retained-frame limit");
        }

        D3D11_TEXTURE2D_DESC stagingDesc{};
        stagingDesc.Width = static_cast<UINT>(size.Width);
        stagingDesc.Height = static_cast<UINT>(size.Height);
        stagingDesc.MipLevels = 1;
        stagingDesc.ArraySize = 1;
        stagingDesc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
        stagingDesc.SampleDesc.Count = 1;
        stagingDesc.Usage = D3D11_USAGE_STAGING;
        stagingDesc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
        com_ptr<ID3D11Texture2D> staging;
        check_hresult(d3dDevice->CreateTexture2D(&stagingDesc, nullptr, staging.put()));

        std::mutex contextMutex;
        std::mutex resultMutex;
        std::condition_variable ready;
        std::vector<std::vector<uint8_t>> frames;
        frames.reserve(static_cast<size_t>(requestedFrames));
        auto pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            captureDevice, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, size);
        auto session = pool.CreateCaptureSession(item);
        auto token = pool.FrameArrived([&](const Direct3D11CaptureFramePool& sender, const winrt::Windows::Foundation::IInspectable&) {
            std::unique_lock resultLock(resultMutex);
            if (frames.size() >= static_cast<size_t>(requestedFrames)) return;
            auto frame = sender.TryGetNextFrame();
            if (!frame) return;
            auto access = frame.Surface().as<::Windows::Graphics::DirectX::Direct3D11::IDirect3DDxgiInterfaceAccess>();
            com_ptr<ID3D11Texture2D> source;
            if (FAILED(access->GetInterface(__uuidof(ID3D11Texture2D), source.put_void()))) return;

            D3D11_TEXTURE2D_DESC sourceDesc{};
            source->GetDesc(&sourceDesc);
            if (sourceDesc.Width != stagingDesc.Width || sourceDesc.Height != stagingDesc.Height) return;

            D3D11_MAPPED_SUBRESOURCE mapped{};
            {
                std::scoped_lock lock(contextMutex);
                context->CopyResource(staging.get(), source.get());
                if (FAILED(context->Map(staging.get(), 0, D3D11_MAP_READ, 0, &mapped))) return;
                std::vector<uint8_t> pixels(static_cast<size_t>(bytesPerFrame));
                for (int row = 0; row < size.Height; ++row) {
                    std::memcpy(
                        pixels.data() + static_cast<size_t>(row) * size.Width * 4,
                        static_cast<const uint8_t*>(mapped.pData) + static_cast<size_t>(row) * mapped.RowPitch,
                        static_cast<size_t>(size.Width) * 4);
                }
                context->Unmap(staging.get(), 0);
                frames.push_back(std::move(pixels));
            }
            if (frames.size() >= static_cast<size_t>(requestedFrames)) ready.notify_one();
        });

        const auto captureStarted = std::chrono::steady_clock::now();
        session.StartCapture();
        {
            std::unique_lock lock(resultMutex);
            if (!ready.wait_for(lock, std::chrono::seconds(10), [&] {
                    return frames.size() >= static_cast<size_t>(requestedFrames);
                })) {
                throw std::runtime_error("requested live frames did not arrive within 10 seconds (window may be protected, capture-blocked, or resizing)");
            }
        }
        session.Close();
        pool.FrameArrived(token);
        pool.Close();
        const auto elapsed = std::chrono::duration<double>(
            std::chrono::steady_clock::now() - captureStarted).count();
        for (int index = 0; index < requestedFrames; ++index) {
            std::wstring path(argv[2]);
            if (requestedFrames > 1) {
                const auto extension = path.find_last_of(L'.');
                const auto insertAt = extension == std::wstring::npos ? path.size() : extension;
                wchar_t suffix[16]{};
                swprintf_s(suffix, L"-%04d", index);
                path.insert(insertAt, suffix);
            }
            write_bmp(path, size.Width, size.Height, frames[static_cast<size_t>(index)]);
        }
        std::wcout << L"CAPTURED hwnd=" << argv[1] << L" frames=" << requestedFrames
                   << L" size=" << size.Width << L"x" << size.Height
                   << L" seconds=" << elapsed << L" fps=" << (requestedFrames / elapsed)
                   << L" output=" << argv[2] << L"\n";
        return 0;
    } catch (const hresult_error& error) {
        std::wcerr << L"WinRT HRESULT 0x" << std::hex << static_cast<uint32_t>(error.code())
                   << L": " << error.message().c_str() << L"\n";
    } catch (const std::exception& error) {
        std::cerr << "capture failed: " << error.what() << "\n";
    }
    return 1;
}
