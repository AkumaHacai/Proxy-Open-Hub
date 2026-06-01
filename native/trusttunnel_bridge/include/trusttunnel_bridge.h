#pragma once

#ifdef _WIN32
#define TT_BRIDGE_EXPORT __declspec(dllexport)
#else
#define TT_BRIDGE_EXPORT
#endif

extern "C" {
TT_BRIDGE_EXPORT int tt_bridge_start(const char* toml_config);
TT_BRIDGE_EXPORT int tt_bridge_stop();
}
