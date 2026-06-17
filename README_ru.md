# Proxy Open Hub

![Логотип Proxy Open Hub](logo/proxy-open-hub-horizontal.svg)

Proxy Open Hub — модульный Windows-клиент для прокси/VPN-ядер. Текущий активный путь проекта:

- Rust backend для адаптеров ядер, доверенных источников, генерации runtime-конфигов, запуска процессов, логов и security-проверок.
- Flutter desktop UI для основного окна, настроек, логов и будущих per-core экранов.
- Bundled TrustTunnel CLI core как первое рабочее ядро.

Это не официальный клиент TrustTunnel. Это независимая оболочка, которая сейчас поддерживает TrustTunnel-профили и готовится к optional-ядрам: sing-box, NaiveProxy, Xray-core, Hysteria2.

## Структура

```text
Cargo.toml                         корень Rust workspace
crates/                            Rust backend crates
  poh_cli/                         CLI-мост для Flutter
  poh_core/                        адаптеры, TrustTunnel parser/builder, security policy
  poh_core_runner/                 материализация runtime config
  poh_core_session/                helpers запуска процессов
  poh_core_store/                  trusted install/verify store для ядер
apps/desktop_flutter/              Flutter desktop application
core-registry/trusted-sources.json реестр доверенных источников ядер
native/bundled/win-x64/            локальные runtime-файлы TrustTunnel
logo/                              исходники логотипа
docs/                              планы миграции и security notes
backups/Old Files/                 старый WPF-клиент и reference-файлы
```

## Быстрый старт

Из корня репозитория:

```powershell
.\scripts\check.ps1
.\scripts\build-desktop.ps1
.\scripts\run-desktop.ps1
```

Готовый exe после сборки:

```text
apps/desktop_flutter/build/windows/x64/runner/Release/proxy_open_hub.exe
```

`build-desktop.ps1` собирает Rust `poh_cli.exe`, Flutter Windows app и копирует backend рядом с приложением.

## TrustTunnel core

Runtime-файлы ожидаются локально:

```text
native/bundled/win-x64/trusttunnel_client.exe
native/bundled/win-x64/wintun.dll
```

В обычном запуске путь к ядру закреплен рядом с приложением и проверяется по pinned SHA-256. Override через `POH_TRUSTTUNNEL_CORE_PATH` разрешен только для debug/dev запуска:

```powershell
$env:POH_DEV = "1"
$env:POH_TRUSTTUNNEL_CORE_PATH = "C:\path\to\trusttunnel_client.exe"
```

## Что уже есть

- Загрузка сохраненных TrustTunnel-профилей из `%LOCALAPPDATA%\ProxyOpenHub\desktop-state.json`.
- Импорт TrustTunnel TOML / `tt://` через Rust backend.
- Генерация TrustTunnel TOML с подстановкой secret refs.
- Запуск/остановка/status/log реального `trusttunnel_client.exe` через Rust CLI.
- Flutter UI с compact/expanded режимами.
- Settings shell, logs shell и import shell в Flutter.
- Live network metrics по OS counters.
- Реестр доверенных источников для будущих optional-ядер.
- Core store умеет выводить установленные ядра и хранить active version на каждое ядро.
- Для включения download UI GitHub-release ядра теперь обязаны иметь `pinned_release`.
- Security hardening: pinned bundled core hash, DPAPI `ProtectedSecrets`, stdin import, runtime cleanup, restrictive ACL, PID identity check, pre-save warning'и для insecure TLS и LAN listener, редакция логов по значениям секретов.

## Что еще в работе

- Полный Flutter-редактор routing/profile settings.
- Trusted download/update UI для sing-box, NaiveProxy, Xray-core, Hysteria2.
- Installer, tray behavior и packaging.

## Проверка

```powershell
.\scripts\check.ps1
.\scripts\build-desktop.ps1 -RustProfile debug
```

## Лицензия

Исходный код Proxy Open Hub распространяется по Apache License 2.0. См. `LICENSE.txt`.

Bundled native components и будущие downloadable cores сохраняют собственные лицензии. См. `NOTICE.md` и license-файлы рядом с каждым native binary.
