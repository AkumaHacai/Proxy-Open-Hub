# Modularity Architecture Progress

Дата старта: 2026-06-18

Цель: перевести Proxy Open Hub на модульную архитектуру, где адаптеры остаются нашим Rust-кодом, а бинарники ядер устанавливаются, проверяются и обновляются через управляемую корзину.

## Phase A — фундамент модульности

- [x] A1. Добавить модель `pinned_release` в trusted source catalog.
- [x] A1. Валидировать pinned release: версия, asset name, SHA-256, allowed asset pattern.
- [x] A1. Запретить installable GitHub source без `pinned_release`.
- [x] A1. Сверять installed manifest с pinned release перед verify/start.
- [x] A2. Добавить `CoreStore::list_installed()`.
- [x] A2. Добавить `active.json` и `CoreStore::active_version()` / `set_active_version()`.
- [x] A2. Делать установленную версию активной после успешного install.
- [x] A2. Добавить CLI `poh_cli core-list-installed`.
- [x] A2. Добавить zip/multifile install с zip-slip guard.
- [x] A2. Разделить SHA pinned archive и hashes установленных файлов для verify/tamper detection.
- [ ] A2. Добавить GC старых версий: active + rollback.
- [ ] A3. Добавить HTTPS downloader: только pinned GitHub asset + SHA-256 verify.
- [ ] A4. Ввести `SessionManager` с состояниями, lock, readiness probe, watchdog.
- [ ] A5. Ввести `CoreLaunchDescriptor` и убрать TrustTunnel-specific запуск из desktop session слоя.

## Phase B — TrustTunnel как управляемый модуль

- [ ] Перенести bundled TrustTunnel в store layout `cores/trusttunnel/<version>/`.
- [ ] Сохранить pinned SHA checks для `trusttunnel_client.exe` и `wintun.dll`.
- [ ] Запускать TrustTunnel через общий descriptor/session слой.
- [ ] Оставить миграционный путь для текущего app-local bundle до появления официального download source.

## Phase C — NaiveProxy как первый скачиваемый модуль

- [ ] Зафиксировать pinned release: version, asset name, SHA-256.
- [ ] Добавить `NaiveProxyAdapter` в `poh_core`.
- [ ] Импортировать `config.json` и proxy URL без хранения пароля в профиле.
- [ ] Материализовать `config.json` через secret placeholder + DPAPI secret store.
- [ ] Подключить install flow через catalog/store/downloader.
- [ ] Подключить LocalProxy readiness probe и system proxy rollback.

## Уже связано с прежней работой

- Секреты остаются в DPAPI `ProtectedSecrets`; новая модульность не возвращает plaintext state.
- Runtime/session/state файлы продолжают получать restrictive ACL.
- Trusted-source policy теперь строже: скачиваемое ядро должно иметь pinned release до включения install UI.
- Fake/swapped core protection усилена: installed manifest теперь сверяется не только с owner/repo/pattern/SHA, но и с pinned version/asset/SHA.

## Следующий безопасный шаг

Реализовать zip/multifile install в `poh_core_store` без сетевого downloader. Это даст основу для NaiveProxy/sing-box архивов и не включит автоматические скачивания раньше проверки pin-ов.
