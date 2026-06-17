# Done — security pass from LLM_Cloud

Дата: 2026-06-18

## Сделано

- Прочитан аудит из `LLM_Cloud`: `todo.md`, `security-findings.md`, `security.md`, `instruct.md`.
- Закрыта основная часть T-1/F-1 для bundled TrustTunnel:
  - убран поиск `trusttunnel_client.exe` по CWD и родительским каталогам;
  - запуск теперь ищет bundled core только рядом с приложением: `native/bundled/win-x64/trusttunnel_client.exe`;
  - добавлена проверка SHA-256 для `trusttunnel_client.exe`;
  - добавлена проверка SHA-256 для `wintun.dll`;
  - `POH_TRUSTTUNNEL_CORE_PATH` разрешен только для dev/debug запуска.
- Обновлен `scripts/build-desktop.ps1`: при сборке Flutter рядом с приложением копируется bundled TrustTunnel core и sidecar-файлы.
- Частично закрыта T-2/F-2:
  - `desktop-state.json` больше не хранит новые секреты plaintext: значения идут в DPAPI `ProtectedSecrets`;
  - legacy plaintext `Secrets` мигрирует в `ProtectedSecrets` при загрузке state;
  - добавлены рестриктивные ACL для state/runtime/config/session/log файлов;
  - Flutter import больше не пишет `tt://`/TOML payload во временный файл;
  - `poh_cli desktop-import-profile -` принимает импорт через stdin;
  - добавлен лимит размера импорта;
  - добавлен лимит размера `desktop-state.json`;
  - `runtime_dir` очищается при раннем падении ядра;
  - старые runtime-каталоги очищаются перед новым стартом, если активной сессии нет.
- Частично закрыта T-3/F-3:
  - Windows status/stop больше не проверяет PID через `stdout.contains`;
  - `tasklist` разбирается как CSV;
  - PID сверяется с ожидаемым образом `trusttunnel_client.exe` перед `taskkill`.
- Частично закрыты T-4/F-4 и T-5/F-5:
  - TrustTunnel adapter выдает `ValidationWarning` для `skip_verification`;
  - предупреждение выдается для кастомного TLS certificate;
  - предупреждение выдается для SOCKS/HTTP listener, открытого в LAN или не на loopback;
  - Flutter запускает preview до сохранения и требует подтверждение для high-risk warnings;
  - профили с отключенной TLS verification / custom certificate получают постоянный UI-индикатор.
- Частично закрыта T-6/F-6:
  - session logs редактируются через `Redactor::redact_secrets` с фактическими секретами профиля;
  - redactor теперь ловит `endpoint.password = ...` и JSON-подобные `client_random: ...`, а не только строки, начинающиеся с ключа.
- Частично закрыты T-7/F-9:
  - добавлены лимиты на импорт и state-файл;
  - `validate_relative_path` блокирует Windows reserved device names (`CON`, `NUL`, `COM1`, `LPT1` и т.п.);
  - TOML builder экранирует tabs и прочие control chars;
  - `unique_profile_id` больше не использует бесконечный suffix loop.
- Обновлен `.gitignore`:
  - IDE/cache/temp/diagnostic artifacts;
  - `LLM_Cloud/` как локальный аудит;
  - `skills/`, `artifacts/`, `backups/`, Flutter/Rust/build мусор.

## Проверено

- `cargo fmt --all`
- `cargo test --workspace`
- `flutter analyze` в `apps/desktop_flutter`
- `scripts/check.ps1`
- `scripts/build-desktop.ps1 -RustProfile debug`
- Smoke test: `poh_cli.exe desktop-import-profile -` принимает импорт через stdin и возвращает security warnings. Тестовый профиль после проверки удален из локального state.
- Smoke test: `poh_cli.exe desktop-preview-profile -` возвращает warnings без записи state.
- Smoke test: импорт во временный `LOCALAPPDATA` пишет DPAPI `ProtectedSecrets`; plaintext secret в state не найден.

## Осталось

- Уйти от `tasklist`/`taskkill` к долгоживущему session manager с handle процесса, если приложение станет daemon/service.
- Перевести flat TOML parser и wildcard matching на проверенные crates (`toml`, `globset`) перед включением download/update UI.
- Добавить `cargo audit` / `cargo deny` в CI после выбора политики зависимостей.
- Добавить Authenticode/publisher validation для release-download ядер перед включением auto-install UI.
