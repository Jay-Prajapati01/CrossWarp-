#include "frame_buffer.h"

#include <algorithm>
#include <array>
#include <atomic>
#include <cstdint>
#include <iostream>
#include <thread>
#include <vector>

namespace {
std::atomic<int> failures = 0;

void check(bool condition, const char* message) {
    if (!condition) {
        std::cerr << "FAIL: " << message << '\n';
        failures.fetch_add(1, std::memory_order_relaxed);
    }
}

void rejects_invalid_frames() {
    crosswarp::idd::DisplayFrameBuffer buffer;
    const std::array<std::uint8_t, 64> pixels{};

    check(!buffer.publish(2, 2, DXGI_FORMAT_B8G8R8A8_UNORM, nullptr, 8), "reject null pixels");
    check(!buffer.publish(2, 2, DXGI_FORMAT_R8G8B8A8_UNORM, pixels.data(), 8), "reject unsupported format");
    check(!buffer.publish(2, 2, DXGI_FORMAT_B8G8R8A8_UNORM, pixels.data(), 7), "reject short source stride");
    check(!buffer.publish(2, 2, DXGI_FORMAT_B8G8R8A8_UNORM, pixels.data(), 8 + 4097), "reject excessive row padding");
    check(!buffer.publish(crosswarp::idd::DisplayFrameBuffer::max_width + 1, 1,
        DXGI_FORMAT_B8G8R8A8_UNORM, pixels.data(), 4), "reject excessive width");

    crosswarp::idd::DisplayFrameInfo info{};
    std::vector<std::uint8_t> copy;
    check(!buffer.snapshot(info, copy), "do not expose a frame before first publish");
    check(!buffer.publish(2, 2, DXGI_FORMAT_B8G8R8A8_UNORM, pixels.data(), 8),
        "capture is disabled until an authorized consumer enables it");
}

void copies_rows_without_source_padding() {
    crosswarp::idd::DisplayFrameBuffer buffer;
    buffer.set_enabled(true);
    const std::array<std::uint8_t, 24> source{
        1, 2, 3, 4, 5, 6, 7, 8, 90, 91, 92, 93,
        11, 12, 13, 14, 15, 16, 17, 18, 80, 81, 82, 83,
    };
    check(buffer.publish(2, 2, DXGI_FORMAT_B8G8R8A8_UNORM, source.data(), 12), "accept a padded BGRA frame");

    crosswarp::idd::DisplayFrameInfo info{};
    std::vector<std::uint8_t> copy;
    check(buffer.snapshot(info, copy), "snapshot the published frame");
    check(info.width == 2 && info.height == 2 && info.stride == 8, "publish tightly packed frame metadata");
    check(info.sequence == 1 && info.captured_at_millis > 0, "publish frame sequence and capture time");
    const std::vector<std::uint8_t> expected{1, 2, 3, 4, 5, 6, 7, 8, 11, 12, 13, 14, 15, 16, 17, 18};
    check(copy == expected, "copy only pixel bytes from each padded source row");

    copy[0] = 250;
    std::vector<std::uint8_t> second_copy;
    check(buffer.snapshot(info, second_copy) && second_copy.front() == 1, "snapshot does not alias internal storage");
}

void snapshots_never_observe_partially_written_frames() {
    crosswarp::idd::DisplayFrameBuffer buffer;
    buffer.set_enabled(true);
    constexpr std::uint32_t width = 64;
    constexpr std::uint32_t height = 64;
    constexpr std::uint32_t stride = width * 4;
    std::vector<std::uint8_t> source(static_cast<std::size_t>(stride) * height);
    std::atomic<bool> finished = false;
    std::thread writer([&] {
        for (std::uint8_t marker = 1; marker <= 40; ++marker) {
            std::fill(source.begin(), source.end(), marker);
            check(buffer.publish(width, height, DXGI_FORMAT_B8G8R8A8_UNORM, source.data(), stride), "publish concurrent frame");
        }
        finished.store(true, std::memory_order_release);
    });

    do {
        crosswarp::idd::DisplayFrameInfo info{};
        std::vector<std::uint8_t> snapshot;
        if (buffer.snapshot(info, snapshot) && !snapshot.empty()) {
            const auto marker = snapshot.front();
            check(std::all_of(snapshot.begin(), snapshot.end(), [marker](auto value) { return value == marker; }),
                "reader sees one complete frame, never a partial update");
            check(info.sequence > 0, "concurrent snapshot has a nonzero sequence");
        }
    } while (!finished.load(std::memory_order_acquire));
    writer.join();
}

void disabling_capture_scrubs_and_hides_the_last_frame() {
    crosswarp::idd::DisplayFrameBuffer buffer;
    buffer.set_enabled(true);
    const std::array<std::uint8_t, 16> source{
        21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36,
    };
    check(buffer.publish(2, 2, DXGI_FORMAT_B8G8R8A8_UNORM, source.data(), 8), "publish while capture is enabled");

    crosswarp::idd::DisplayFrameInfo info{};
    std::vector<std::uint8_t> copy;
    check(buffer.snapshot(info, copy) && copy == std::vector<std::uint8_t>(source.begin(), source.end()),
        "snapshot is available while capture is enabled");

    buffer.set_enabled(false);
    copy.clear();
    check(!buffer.snapshot(info, copy), "disabled capture hides cached pixels");
    check(!buffer.publish(2, 2, DXGI_FORMAT_B8G8R8A8_UNORM, source.data(), 8), "disabled capture rejects new frames");

    buffer.set_enabled(true);
    check(!buffer.snapshot(info, copy), "reenabling does not expose a stale frame");
}
} // namespace

int main() {
    rejects_invalid_frames();
    copies_rows_without_source_padding();
    snapshots_never_observe_partially_written_frames();
    disabling_capture_scrubs_and_hides_the_last_frame();
    const int failure_count = failures.load(std::memory_order_relaxed);
    if (failure_count != 0) {
        std::cerr << failure_count << " frame-buffer contract check(s) failed\n";
        return 1;
    }
    std::cout << "IDD frame-buffer contract checks passed\n";
    return 0;
}
