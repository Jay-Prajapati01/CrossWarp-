#include "window_capture.h"
#include <cstdint>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <vector>
#include <Windows.h>

#pragma pack(push, 1)
struct BmpFileHeader { uint16_t type{0x4d42}; uint32_t size{}; uint16_t reserved1{}; uint16_t reserved2{}; uint32_t offset{54}; };
struct BmpInfoHeader { uint32_t size{40}; int32_t width{}; int32_t height{}; uint16_t planes{1}; uint16_t bits{32}; uint32_t compression{}; uint32_t imageSize{}; int32_t xppm{}; int32_t yppm{}; uint32_t colors{}; uint32_t important{}; };
#pragma pack(pop)

int wmain(int argc, wchar_t** argv) {
    if (argc != 3) {
        std::wcerr << L"Usage: crosswarp-window-capture-smoke.exe <HWND> <output.bmp>\n";
        return 2;
    }
    void* capture = nullptr;
    try {
        const auto hwnd = std::wcstoull(argv[1], nullptr, 0);
        const auto started = cw_capture_start(hwnd, &capture);
        if (started != 0) throw std::runtime_error(cw_capture_last_error());
        std::vector<uint8_t> pixels;
        uint32_t width = 0, height = 0;
        uint64_t sequence = 0;
        size_t required = 0;
        const auto deadline = GetTickCount64() + 5000;
        for (;;) {
            auto result = cw_capture_read(capture, 0, pixels.data(), pixels.size(),
                &required, &width, &height, &sequence);
            if (result == 1) break;
            if (result == -2) {
                if (!required || required > 64u * 1024u * 1024u) throw std::runtime_error("frame byte count is invalid");
                pixels.resize(required);
            } else if (result < 0) {
                throw std::runtime_error(cw_capture_last_error());
            } else if (GetTickCount64() >= deadline) {
                throw std::runtime_error("no frame arrived within 5 seconds");
            } else {
                Sleep(10);
            }
        }
        BmpFileHeader fileHeader;
        BmpInfoHeader infoHeader;
        infoHeader.width = static_cast<int32_t>(width);
        infoHeader.height = -static_cast<int32_t>(height);
        infoHeader.bits = 32;
        infoHeader.imageSize = static_cast<uint32_t>(pixels.size());
        fileHeader.size = sizeof(fileHeader) + sizeof(infoHeader) + infoHeader.imageSize;
        std::ofstream file(argv[2], std::ios::binary);
        file.write(reinterpret_cast<const char*>(&fileHeader), sizeof(fileHeader));
        file.write(reinterpret_cast<const char*>(&infoHeader), sizeof(infoHeader));
        file.write(reinterpret_cast<const char*>(pixels.data()), static_cast<std::streamsize>(pixels.size()));
        if (!file) throw std::runtime_error("could not save captured frame");
        std::wcout << L"CAPTURED hwnd=" << argv[1] << L" seq=" << sequence
                   << L" size=" << width << L"x" << height << L" bytes=" << pixels.size() << L"\n";
        cw_capture_stop(capture);
        return 0;
    } catch (const std::exception& e) {
        if (capture) cw_capture_stop(capture);
        std::cerr << "capture failed: " << e.what() << "\n";
        return 1;
    }
}
