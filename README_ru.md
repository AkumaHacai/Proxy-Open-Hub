# Proxy Open Hub

![Логотип Proxy Open Hub](logo/proxy-open-hub-horizontal.svg)

Proxy Open Hub — модульный Windows-хаб для прокси/VPN-ядер. Текущее приложение
собрано на Rust backend и Flutter desktop UI, а первым рабочим ядром поставляется
bundled TrustTunnel runtime.

Это не официальный клиент TrustTunnel. Это независимая оболочка для управления
разными ядрами через один интерфейс. TrustTunnel сейчас входит в portable-сборку;
sing-box, NaiveProxy, Xray-core и Hysteria2 уже отражены в модели ядер и UI, но их
trusted install/update flow ещё доводится.

English README: [README.md](README.md)

## Portable pre-release

Текущий portable-пакет для Windows x64:

```text
ProxyOpenHub-portable-win-x64-2026-06-19-fix1.zip
```

Внутри архива:

```text
proxy_open_hub.exe          Flutter desktop app
poh_cli.exe                 Rust CLI bridge для UI
data/                       runtime-данные Flutter
native/bundled/win-x64/     bundled TrustTunnel client и Wintun DLL
README.txt                  заметки portable-пакета
```

Как запустить:

1. Скачать zip из GitHub Releases.
2. Распаковать всю папку `ProxyOpenHub-portable-win-x64`.
3. Запустить `proxy_open_hub.exe`.

Состояние приложения хранится здесь:

```text
%LOCALAPPDATA%\ProxyOpenHub\
```

Секреты сохраняются через Windows DPAPI и привязаны к текущему Windows-user.
Это pre-release: возможны предупреждения Windows для неподписанных бинарников,
нет полноценного installer/tray flow и обновление пока ручное.

## Структура проекта

```text
Cargo.toml                         корень Rust workspace
crates/                            Rust backend crates
  poh_cli/                         CLI-мост для Flutter
  poh_core/                        адаптеры ядер, import/parsing, security policy
  poh_core_runner/                 материализация runtime config
  poh_core_session/                запуск процессов и launch descriptors
  poh_core_store/                  trusted install/verify store для ядер
apps/desktop_flutter/              Flutter desktop application
core-registry/trusted-sources.json реестр доверенных источников ядер
native/bundled/win-x64/            bundled runtime-файлы TrustTunnel
logo/                              исходники логотипа
docs/                              migration/security/native notes
backups/Old Files/                 старый WPF app, references, архив
```

Старый WPF/.NET проект и reference-файлы лежат в `backups/Old Files/`.

## Быстрый старт для разработки

Из корня репозитория:

```powershell
.\scripts\check.ps1
.\scripts\build-desktop.ps1
.\scripts\run-desktop.ps1
```

`build-desktop.ps1` собирает Rust CLI и Flutter Windows app, затем копирует
`poh_cli.exe` рядом с `proxy_open_hub.exe`, чтобы UI нашёл backend.

Готовый desktop executable:

```text
apps/desktop_flutter/build/windows/x64/runner/Release/proxy_open_hub.exe
```

Bundled TrustTunnel runtime ожидается здесь:

```text
native/bundled/win-x64/trusttunnel_client.exe
native/bundled/win-x64/wintun.dll
```

Для локальной отладки можно переопределить путь к backend:

```powershell
$env:POH_CLI_PATH = "C:\path\to\poh_cli.exe"
```

Override TrustTunnel core специально ограничен debug/dev запуском и всё равно
должен проходить pinned SHA-256 там, где это требует security policy:

```powershell
$env:POH_DEV = "1"
$env:POH_TRUSTTUNNEL_CORE_PATH = "C:\path\to\trusttunnel_client.exe"
```

## Сборка и проверка

Rust backend:

```powershell
C:\Users\mirot\.cargo\bin\cargo.exe fmt --all --check
C:\Users\mirot\.cargo\bin\cargo.exe clippy --workspace -- -D warnings
C:\Users\mirot\.cargo\bin\cargo.exe test --workspace
C:\Users\mirot\.cargo\bin\cargo.exe build -p poh_cli
```

Flutter UI:

```powershell
cd .\apps\desktop_flutter
C:\Users\mirot\devtools\flutter\bin\dart.bat format lib test
C:\Users\mirot\devtools\flutter\bin\flutter.bat analyze
C:\Users\mirot\devtools\flutter\bin\flutter.bat test
C:\Users\mirot\devtools\flutter\bin\flutter.bat build windows
```

Общие scripts:

```powershell
.\scripts\check.ps1
.\scripts\build-desktop.ps1
.\scripts\run-desktop.ps1 -Build
```

## CLI-мост

Flutter общается с Rust через `poh_cli.exe`. Основные desktop-команды:

```text
desktop-list-profiles <state-path>
desktop-core-schema <core_id>
desktop-core-modes <core_id>
desktop-validate-profile <state-path>    # JSON on stdin
desktop-update-profile <state-path>      # JSON on stdin
desktop-preview-profile <input-text-file|->
desktop-import-profile <input-text-file|->
desktop-session-plan <state-path> <profile-id>
desktop-session-start <state-path> <profile-id>
desktop-session-supervise <state-path> <profile-id>
desktop-session-stop
desktop-session-reset
desktop-session-status
desktop-session-log
```

Per-core route settings хранятся в `desktop-state.json` через
`RoutePresetsByCore`, `ActiveRouteByCore` и `ActiveModeByCore`.

## Текущий статус

Реализовано в Rust + Flutter ветке:

- Загрузка desktop-профилей из `%LOCALAPPDATA%\ProxyOpenHub\desktop-state.json`.
- Импорт TrustTunnel TOML и `tt://` с DPAPI-защищёнными секретами.
- NaiveProxy JSON/proxy URL import path и generic per-core profile model.
- Per-core список профилей, фильтр по активному ядру и core-aware accent colors.
- Schema-driven profile editor через `desktop-core-schema`,
  `desktop-validate-profile` и `desktop-update-profile`.
- Per-core route modes и user route presets по desktop-state contract.
- TrustTunnel session plan/start/stop/status/log lifecycle через Rust CLI.
- Supervised session launch path, lifecycle states, single-instance lock,
  readiness probes и faulted-session reporting.
- Redacted runtime previews и log redaction.
- System proxy, DNS, routes, firewall и kill-switch rollback ledger.
- Flutter main UI: compact/expanded modes, settings, routes, logs, import,
  profile editing, live network metrics и core tabs.
- Trusted-source registry и core store foundation для будущих downloadable cores.
- ZIP/multifile core artifacts с zip-slip guards и hashes установленных файлов.

Ещё в работе:

- Полный trusted download/update UI для sing-box, NaiveProxy, Xray-core и
  Hysteria2.
- Exact TrustTunnel/Wintun adapter matching для live traffic.
- Streaming logs вместо manual refresh.
- Per-core advanced settings pages.
- Installer, tray behavior, code signing и automatic update flow.
- Long-lived service/watchdog behavior и более широкие integration tests.

## Лицензия

Исходный код Proxy Open Hub распространяется по Apache License 2.0. См.
`LICENSE.txt`.

Bundled native components и будущие downloadable cores сохраняют собственные
лицензии. См. `NOTICE.md` и license-файлы рядом с каждым native binary.
