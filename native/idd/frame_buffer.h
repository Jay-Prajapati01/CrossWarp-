#pragma once

#ifndef NOMINMAX
#define NOMINMAX
#endif

#include <Windows.h>
#include <dxgiformat.h>

#include <cstdint>
#include <cstring>
#include <atomic>
#include <limits>
#include <mutex>
#include <vector>

namespace crosswarp {
namespace idd {

struct DisplayFrameInfo {
    std::uint32_t width{};
    std::uint32_t height{};
    std::uint32_t stride{};
    std::uint32_t pixel_format{};
    std::uint64_t sequence{};
    std::uint64_t captured_at_millis{};
};

// Thread-safe, single-frame CPU snapshot used by the IDD swapchain worker.
// It does not expose the pixels outside the UMDF driver host.
class DisplayFrameBuffer {
public:
    static constexpr std::uint32_t max_width = 3840;
    static constexpr std::uint32_t max_height = 2160;
    static constexpr std::uint32_t max_row_padding = 4096;

    bool is_enabled() const noexcept {
        return enabled_.load(std::memory_order_acquire);
    }

    void set_enabled(bool enabled) noexcept {
        if (enabled) {
            enabled_.store(true, std::memory_order_release);
            return;
        }

        enabled_.store(false, std::memory_order_release);
        try {
            std::lock_guard<std::mutex> guard(mutex_);
            if (!pixels_.empty()) {
                SecureZeroMemory(pixels_.data(), pixels_.size());
                pixels_.clear();
            }
            info_ = {};
            ready_ = false;
        } catch (...) {
            // A failed lock leaves capture disabled; no new snapshots are exposed.
        }
    }

    bool publish(std::uint32_t width, std::uint32_t height, DXGI_FORMAT format,
                 const void* pixels, std::uint32_t source_stride) noexcept {
        if (!enabled_.load(std::memory_order_acquire) || pixels == nullptr || width == 0 || height == 0 || width > max_width || height > max_height ||
            format != DXGI_FORMAT_B8G8R8A8_UNORM) {
            return false;
        }

        const std::size_t row_bytes = static_cast<std::size_t>(width) * 4;
        if (source_stride < row_bytes || source_stride - row_bytes > max_row_padding ||
            row_bytes > (std::numeric_limits<std::size_t>::max)() / height) {
            return false;
        }
        const std::size_t frame_bytes = row_bytes * height;

        try {
            std::lock_guard<std::mutex> guard(mutex_);
            if (!enabled_.load(std::memory_order_acquire)) {
                return false;
            }
            pixels_.resize(frame_bytes);
            const auto* source = static_cast<const std::uint8_t*>(pixels);
            for (std::uint32_t row = 0; row < height; ++row) {
                std::memcpy(pixels_.data() + static_cast<std::size_t>(row) * row_bytes,
                            source + static_cast<std::size_t>(row) * source_stride, row_bytes);
            }
            info_.width = width;
            info_.height = height;
            info_.stride = static_cast<std::uint32_t>(row_bytes);
            info_.pixel_format = static_cast<std::uint32_t>(format);
            info_.sequence = info_.sequence == (std::numeric_limits<std::uint64_t>::max)() ? 1 : info_.sequence + 1;
            info_.captured_at_millis = GetTickCount64();
            ready_ = true;
            return true;
        } catch (...) {
            return false;
        }
    }

    bool snapshot(DisplayFrameInfo& info, std::vector<std::uint8_t>& pixels) const noexcept {
        if (!enabled_.load(std::memory_order_acquire)) {
            return false;
        }
        try {
            std::lock_guard<std::mutex> guard(mutex_);
            if (!enabled_.load(std::memory_order_acquire) || !ready_) {
                return false;
            }
            info = info_;
            pixels = pixels_;
            return true;
        } catch (...) {
            return false;
        }
    }

private:
    mutable std::mutex mutex_;
    std::atomic<bool> enabled_{false};
    DisplayFrameInfo info_{};
    std::vector<std::uint8_t> pixels_;
    bool ready_{};
};

} // namespace idd
} // namespace crosswarp
