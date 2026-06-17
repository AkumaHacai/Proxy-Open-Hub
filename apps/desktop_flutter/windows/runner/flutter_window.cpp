#include "flutter_window.h"

#include <cstdint>
#include <iphlpapi.h>
#include <optional>
#include <string>
#include <variant>
#include <vector>

#include "flutter/generated_plugin_registrant.h"

namespace {

flutter::EncodableValue BuildNetworkMetricsSnapshot() {
  unsigned long long rx_bytes = 0;
  unsigned long long tx_bytes = 0;
  int interfaces = 0;

  ULONG table_size = 0;
  if (GetIfTable(nullptr, &table_size, FALSE) == ERROR_INSUFFICIENT_BUFFER) {
    std::vector<unsigned char> buffer(table_size);
    auto* table = reinterpret_cast<MIB_IFTABLE*>(buffer.data());
    if (GetIfTable(table, &table_size, FALSE) == NO_ERROR) {
      for (DWORD i = 0; i < table->dwNumEntries; ++i) {
        const MIB_IFROW& row = table->table[i];
        if (row.dwOperStatus != IF_OPER_STATUS_OPERATIONAL ||
            row.dwType == IF_TYPE_SOFTWARE_LOOPBACK) {
          continue;
        }

        rx_bytes += row.dwInOctets;
        tx_bytes += row.dwOutOctets;
        ++interfaces;
      }
    }
  }

  flutter::EncodableMap snapshot;
  snapshot[flutter::EncodableValue("rxBytes")] =
      flutter::EncodableValue(static_cast<int64_t>(rx_bytes));
  snapshot[flutter::EncodableValue("txBytes")] =
      flutter::EncodableValue(static_cast<int64_t>(tx_bytes));
  snapshot[flutter::EncodableValue("interfaces")] =
      flutter::EncodableValue(interfaces);
  return flutter::EncodableValue(snapshot);
}

flutter::EncodableValue BuildWindowSizeSnapshot(HWND hwnd) {
  RECT rect;
  GetWindowRect(hwnd, &rect);
  const UINT dpi = GetDpiForWindow(hwnd);
  const double scale = dpi / 96.0;

  flutter::EncodableMap snapshot;
  snapshot[flutter::EncodableValue("width")] =
      flutter::EncodableValue((rect.right - rect.left) / scale);
  snapshot[flutter::EncodableValue("height")] =
      flutter::EncodableValue((rect.bottom - rect.top) / scale);
  return flutter::EncodableValue(snapshot);
}

std::optional<double> ReadDoubleArgument(const flutter::EncodableMap& arguments,
                                         const char* key) {
  const auto iterator = arguments.find(flutter::EncodableValue(key));
  if (iterator == arguments.end()) {
    return std::nullopt;
  }

  if (const auto* value = std::get_if<double>(&iterator->second)) {
    return *value;
  }
  if (const auto* value = std::get_if<int32_t>(&iterator->second)) {
    return static_cast<double>(*value);
  }
  if (const auto* value = std::get_if<int64_t>(&iterator->second)) {
    return static_cast<double>(*value);
  }

  return std::nullopt;
}

bool SetLogicalWindowSize(HWND hwnd, double width, double height) {
  RECT rect;
  if (!GetWindowRect(hwnd, &rect)) {
    return false;
  }

  const UINT dpi = GetDpiForWindow(hwnd);
  const double scale = dpi / 96.0;
  const int physical_width = static_cast<int>(width * scale);
  const int physical_height = static_cast<int>(height * scale);

  return SetWindowPos(hwnd, nullptr, rect.left, rect.top, physical_width,
                      physical_height, SWP_NOZORDER | SWP_NOACTIVATE) == TRUE;
}

}  // namespace

FlutterWindow::FlutterWindow(const flutter::DartProject& project)
    : project_(project) {}

FlutterWindow::~FlutterWindow() {}

bool FlutterWindow::OnCreate() {
  if (!Win32Window::OnCreate()) {
    return false;
  }

  RECT frame = GetClientArea();

  // The size here must match the window dimensions to avoid unnecessary surface
  // creation / destruction in the startup path.
  flutter_controller_ = std::make_unique<flutter::FlutterViewController>(
      frame.right - frame.left, frame.bottom - frame.top, project_);
  // Ensure that basic setup of the controller was successful.
  if (!flutter_controller_->engine() || !flutter_controller_->view()) {
    return false;
  }
  RegisterPlugins(flutter_controller_->engine());
  window_channel_ =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          flutter_controller_->engine()->messenger(), "proxy_open_hub/window",
          &flutter::StandardMethodCodec::GetInstance());
  window_channel_->SetMethodCallHandler(
      [this](const flutter::MethodCall<flutter::EncodableValue>& call,
             std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>>
                 result) {
        if (call.method_name() == "minimize") {
          ::ShowWindow(GetHandle(), SW_MINIMIZE);
          result->Success();
          return;
        }

        if (call.method_name() == "close") {
          ::PostMessage(GetHandle(), WM_CLOSE, 0, 0);
          result->Success();
          return;
        }

        if (call.method_name() == "startDrag") {
          ::ReleaseCapture();
          ::SendMessage(GetHandle(), WM_NCLBUTTONDOWN, HTCAPTION, 0);
          result->Success();
          return;
        }

        if (call.method_name() == "networkSnapshot") {
          result->Success(BuildNetworkMetricsSnapshot());
          return;
        }

        if (call.method_name() == "getWindowSize") {
          result->Success(BuildWindowSizeSnapshot(GetHandle()));
          return;
        }

        if (call.method_name() == "setWindowSize") {
          const auto* arguments =
              std::get_if<flutter::EncodableMap>(call.arguments());
          if (arguments == nullptr) {
            result->Error("bad_arguments", "Expected width and height.");
            return;
          }

          const auto width = ReadDoubleArgument(*arguments, "width");
          const auto height = ReadDoubleArgument(*arguments, "height");
          if (!width.has_value() || !height.has_value()) {
            result->Error("bad_arguments", "Expected numeric width/height.");
            return;
          }

          if (!SetLogicalWindowSize(GetHandle(), *width, *height)) {
            result->Error("resize_failed", "Unable to resize window.");
            return;
          }

          result->Success();
          return;
        }

        result->NotImplemented();
      });
  SetChildContent(flutter_controller_->view()->GetNativeWindow());

  flutter_controller_->engine()->SetNextFrameCallback([&]() {
    this->Show();
  });

  // Flutter can complete the first frame before the "show window" callback is
  // registered. The following call ensures a frame is pending to ensure the
  // window is shown. It is a no-op if the first frame hasn't completed yet.
  flutter_controller_->ForceRedraw();

  return true;
}

void FlutterWindow::OnDestroy() {
  window_channel_ = nullptr;
  if (flutter_controller_) {
    flutter_controller_ = nullptr;
  }

  Win32Window::OnDestroy();
}

LRESULT
FlutterWindow::MessageHandler(HWND hwnd, UINT const message,
                              WPARAM const wparam,
                              LPARAM const lparam) noexcept {
  // Give Flutter, including plugins, an opportunity to handle window messages.
  if (flutter_controller_) {
    std::optional<LRESULT> result =
        flutter_controller_->HandleTopLevelWindowProc(hwnd, message, wparam,
                                                      lparam);
    if (result) {
      return *result;
    }
  }

  switch (message) {
    case WM_FONTCHANGE:
      flutter_controller_->engine()->ReloadSystemFonts();
      break;
  }

  return Win32Window::MessageHandler(hwnd, message, wparam, lparam);
}
