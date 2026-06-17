# Proxy Open Hub: проект, Rust-часть и проверка готовности

## Что это за проект

Proxy Open Hub - модульный Windows-клиент для прокси/VPN-ядер. Сейчас активная миграция идет от старого WPF/C# клиента к новой структуре:

- Rust отвечает за ядра, импорт конфигов, безопасность, материализацию runtime-конфигов, запуск процессов, логи и проверки.
- Flutter отвечает за интерфейс: главное окно, compact/expanded режимы, настройки, логи и окна импорта.
- TrustTunnel сейчас первое рабочее ядро. Остальные ядра, например sing-box, NaiveProxy, Xray-core и Hysteria2, готовятся как опциональные модули через доверенные источники.

Старый WPF проект и HTML-референсы сохранены в `backups/Old Files/2026-06-16-wpf-and-references/`. Активный клиент запускается из `apps/desktop_flutter`.

## Главная структура

```text
Cargo.toml                         Rust workspace
crates/poh_core                    core adapters, TrustTunnel parser, security policy
crates/poh_core_runner             materialization runtime files + secret substitution
crates/poh_core_session            process launch abstractions
crates/poh_core_store              trusted core install/verify store
crates/poh_cli                     CLI bridge for Flutter
apps/desktop_flutter               Flutter desktop UI
core-registry/trusted-sources.json trusted source registry for downloadable cores
native/bundled/win-x64             bundled TrustTunnel runtime
scripts/check.ps1                  full local verification
scripts/build-desktop.ps1          build Rust CLI + Flutter Windows app
scripts/run-desktop.ps1            run built app with POH_CLI_PATH
```

## Как работает Rust часть

### `poh_core`

Это общий слой модели и адаптеров ядер.

Что уже есть:

- `CoreRegistry` хранит доступные адаптеры.
- `TrustTunnelAdapter` умеет определять и импортировать `tt://` и TrustTunnel TOML.
- `TrustTunnelTomlParser` парсит endpoint, routing, listener TUN/SOCKS, DNS, secrets.
- `TrustTunnelTomlBuilder` собирает runtime TOML обратно.
- Security policy проверяет доверенные источники, runtime-файлы, редактируемые secret-значения и fake/tampered core manifests.

Важный момент: generic `Profile` не хранит реальные секреты. Парсер возвращает секреты отдельно как candidates, дальше они превращаются в `secret://...` ссылки.

### `poh_core_runner`

Материализует runtime-конфиг перед запуском ядра.

Что делает:

- Подставляет секреты через resolver.
- Пишет runtime-файлы только внутрь разрешенной временной директории.
- Блокирует absolute paths, `..`, Windows backslash paths, опасные environment keys и слишком большие generated configs.
- Дает redacted preview, чтобы UI мог показывать конфиг без утечки паролей.

### `poh_core_session`

Низкоуровневый запуск процессов.

Сейчас используется как безопасная модель для будущей унификации запуска разных ядер. В desktop-flow TrustTunnel стартует через `poh_cli`, но session crate уже тестирует launch spec, missing executable и redacted output.

### `poh_core_store`

Будущая база для загрузки и обновления ядер.

Что уже заложено:

- Trusted source registry.
- Проверка source type, owner/repo, asset pattern.
- SHA-256 validation.
- Запрет path traversal при распаковке/установке.
- Проверка установленного executable на tampering.

Пока install/download UI не включен в приложение. Это следующий крупный этап.

### `poh_cli`

CLI-мост между Flutter и Rust.

Основные команды:

```powershell
.\target\debug\poh_cli.exe list
.\target\debug\poh_cli.exe sources
.\target\debug\poh_cli.exe detect "<profile text>"
.\target\debug\poh_cli.exe desktop-import-profile C:\path\to\profile.toml
.\target\debug\poh_cli.exe desktop-session-plan <desktop-state.json> <profile-id>
.\target\debug\poh_cli.exe desktop-session-start <desktop-state.json> <profile-id>
.\target\debug\poh_cli.exe desktop-session-stop
.\target\debug\poh_cli.exe desktop-session-status
.\target\debug\poh_cli.exe desktop-session-log
```

Flutter ищет `poh_cli.exe` рядом с `proxy_open_hub.exe` или через `POH_CLI_PATH`.

## Desktop state и секреты

Новый state:

```text
%LOCALAPPDATA%\ProxyOpenHub\desktop-state.json
```

Старый state, который может быть подхвачен как база:

```text
%LOCALAPPDATA%\TrustTunnel\desktop-state.json
```

Импорт TrustTunnel профиля делает так:

1. Rust парсит `tt://` или TOML.
2. Создает `DesktopProfile`.
3. Endpoint/listener получают только `PasswordSecretRef` / `ClientRandomSecretRef`.
4. Реальные значения шифруются через Windows DPAPI и кладутся в `ProtectedSecrets`.
5. Старый plaintext `Secrets` мигрирует в `ProtectedSecrets` при загрузке state.
6. Flutter перечитывает state и показывает новый сервер.

## Как дебажить Rust

### Быстрая проверка всего проекта

```powershell
.\scripts\check.ps1
```

Скрипт выполняет:

- `cargo fmt --all --check`
- `cargo test --workspace`
- `dart format --set-exit-if-changed`
- `flutter analyze`
- `flutter test`

### Только Rust

```powershell
cargo fmt --all --check
cargo test --workspace
cargo build -p poh_cli
```

### Проверить импорт TOML без порчи реального state

```powershell
$temp = Join-Path $env:TEMP ("poh-debug-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $temp | Out-Null
$old = $env:LOCALAPPDATA
$env:LOCALAPPDATA = $temp

.\target\debug\poh_cli.exe desktop-import-profile C:\path\to\profile.toml
Get-Content "$temp\ProxyOpenHub\desktop-state.json"

$env:LOCALAPPDATA = $old
Remove-Item -Recurse -Force $temp
```

Это лучший способ проверить импорт, не меняя реальные профили пользователя.

### Проверить запуск TrustTunnel

Перед запуском нужен state и profile id:

```powershell
.\target\debug\poh_cli.exe desktop-session-plan "%LOCALAPPDATA%\ProxyOpenHub\desktop-state.json" <profile-id>
.\target\debug\poh_cli.exe desktop-session-start "%LOCALAPPDATA%\ProxyOpenHub\desktop-state.json" <profile-id>
.\target\debug\poh_cli.exe desktop-session-status
.\target\debug\poh_cli.exe desktop-session-log
.\target\debug\poh_cli.exe desktop-session-stop
```

Путь к TrustTunnel core:

```text
native/bundled/win-x64/trusttunnel_client.exe
native/bundled/win-x64/wintun.dll
```

Для локального дебага можно переопределить путь. В обычном запуске это заблокировано; нужен `POH_DEV=1` или debug build, и бинарь всё равно проверяется по pinned SHA-256:

```powershell
$env:POH_DEV = "1"
$env:POH_TRUSTTUNNEL_CORE_PATH = "C:\path\to\trusttunnel_client.exe"
```

## Как собирать и запускать UI

```powershell
.\scripts\build-desktop.ps1
.\scripts\run-desktop.ps1
```

Готовый exe:

```text
apps/desktop_flutter/build/windows/x64/runner/Release/proxy_open_hub.exe
```

Рядом должен лежать:

```text
apps/desktop_flutter/build/windows/x64/runner/Release/poh_cli.exe
```

Если `poh_cli.exe` не найден, Connect/import/logs не смогут говорить с Rust backend.

## Что готово сейчас

- Rust workspace создан и проходит тесты.
- TrustTunnel TOML/deeplink import работает.
- TOML parser принимает UTF-8 BOM файлы из Windows редакторов.
- Flutter Add Server открывает окно импорта и вызывает Rust CLI.
- Flutter грузит реальные профили из `desktop-state.json`.
- Connect/Disconnect вызывает Rust CLI и запускает реальный `trusttunnel_client.exe`.
- Логи читаются через Rust CLI и редактируются редактором секретов.
- Live metrics читаются через OS network counters.
- Настройки приложения сохраняются в `%LOCALAPPDATA%\ProxyOpenHub\app-settings.json`.
- Trusted-source registry и проверки fake/tampered core artifacts заложены для будущих ядер.
- Секреты профилей хранятся как DPAPI `ProtectedSecrets`.
- Import preview показывает TLS/LAN warnings до сохранения и требует подтверждение.

## Что еще не закончено

- Полный routing/profile editor еще нужно перенести из WPF в Flutter.
- Log streaming пока заменен manual refresh.
- Exact TrustTunnel/Wintun adapter matching для traffic metrics еще приблизительный.
- Download/update UI для sing-box, NaiveProxy, Xray-core, Hysteria2 еще не включен.
- Нужны tray, installer, packaging и подпись сборки.

## Легит-чек перед продолжением разработки

Перед тем как считать этап рабочим:

```powershell
.\scripts\check.ps1
.\scripts\build-desktop.ps1
.\scripts\run-desktop.ps1
```

Ручная проверка:

- Открывается главный экран.
- `Add Server` открывает импорт.
- TOML/tt-link импортируется и появляется в списке.
- `desktop-state.json` содержит `PasswordSecretRef`, а не пароль внутри профиля.
- `Connect` запускает процесс или возвращает понятную ошибку.
- `Logs` показывает лог или причину отсутствия активной сессии.
- Theme toggle не моргает чужой палитрой.
- Compact/expanded режим не дает overflow и не ломает центральную кнопку.
