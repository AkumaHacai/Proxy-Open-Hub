# Proxy Open Hub

![Platform](https://img.shields.io/badge/platform-Windows-blue)
![Framework](https://img.shields.io/badge/framework-.NET_10-purple)
![License](https://img.shields.io/badge/license-Apache_2.0-green)
![Status](https://img.shields.io/badge/status-alpha-orange)
![Current core](https://img.shields.io/badge/current_core-TrustTunnel-0f766e)

> Windows desktop proxy/VPN client for TrustTunnel. Planned adapters: sing-box, Xray-core, Hysteria2 and NaiveProxy.
>
> English version: [README.en.md](README.en.md)

Proxy Open Hub — независимый клиент для Windows на WPF и .NET.

По факту это пока **TrustTunnel-first** проект. Рабочий путь сейчас один: импорт профиля, типизированные настройки, генерация конфигурации, запуск TrustTunnel core, диагностика, TUN/SOCKS режимы и системный прокси.

Это не официальный клиент TrustTunnel. Это отдельная оболочка. До реального universal hub проект еще не дошел. Но цель именно такая: один UI для нескольких ядер и нескольких форматов профилей.

## Что это такое

Проект собирает в одном приложении то, что обычно размазано по разным утилитам:

- импорт профилей;
- хранение typed settings;
- генерация конфигов;
- запуск локального ядра;
- переключение listener mode;
- диагностика и базовая телеметрия.

Смысл простой: один desktop client для Windows, а не набор разрозненных launcher'ов и конфигов.

## Состояние проекта

Статус: **alpha**.

Нормально работает только TrustTunnel-сценарий. Остальное пока не готово и не должно продаваться как “уже поддерживается”.

Проблемы, которые есть сейчас:

- нет нормального hardening для хранения секретов;
- нет подписанного установщика и внятного update flow;
- нет стабильного release-пайплайна с готовыми бинарниками;
- нет реального multi-core adapter layer;
- документация пока тонкая;
- скриншоты интерфейса еще не добавлены.

## Ядра

| Core | Status | Notes |
|---|---|---|
| TrustTunnel | working alpha | текущий рабочий core |
| sing-box | planned | нужен adapter, config generator, runtime launcher |
| Xray-core | planned | нужен adapter и модель профиля |
| Hysteria2 | planned | нужен runtime adapter |
| NaiveProxy | planned | нужен runtime adapter |

Сухо и честно: сейчас это не multi-core client. Сейчас это TrustTunnel GUI с заделом под multi-core.

## Что уже есть

- WPF интерфейс с compact и expanded режимами.
- Tray behavior.
- Импорт `tt://` ссылок.
- Ручное создание серверов.
- TOML import / preview / tools.
- Типизированные модели endpoint, listener, routing и server profile.
- Routing presets.
- Валидация host:port, DNS upstream, CIDR, MTU, certificate fields.
- SOCKS5 credentials generation.
- System proxy toggle для SOCKS5.
- Live network counters.
- Server diagnostics.
- Локализация: English, Russian, Chinese, Persian.

## Чего еще нет

- Core selector внутри профиля.
- Унифицированный adapter layer для нескольких ядер.
- Безопасное постоянное хранилище секретов уровня DPAPI / Windows Credential Manager.
- Подписанные portable / release сборки.
- Нормальная документация по каждому ядру.
- Импорт JSON / YAML / URL форматов для sing-box, Xray и других ядер.
- Скриншоты, GIF и короткая демо-запись.

## Дорожная карта

### Архитектура

- [ ] Ввести `CoreKind`.
- [ ] Ввести общий `IProxyCoreAdapter`.
- [ ] Вынести TrustTunnel runtime код в отдельный adapter.
- [ ] Отвязать UI от прямого создания TrustTunnel controller.
- [ ] Добавить manifest для core binaries: version, source, SHA256, license.

### Новые ядра

- [ ] sing-box
- [ ] Xray-core
- [ ] Hysteria2
- [ ] NaiveProxy

### Перед стабильным релизом

- [ ] DPAPI / Windows Credential Manager для секретов.
- [ ] Signed installer.
- [ ] Trusted binary verification.
- [ ] QA для TUN, SOCKS5, HTTP proxy, service mode и tray logic.
- [ ] Release notes и changelog.

## Установка

Готовых стабильных бинарников пока нет. Когда появятся — они будут лежать в `Releases`.

Сейчас вариант один: собрать из исходников.

### Требования

- Windows 10 или Windows 11
- .NET SDK 10
- нативные файлы TrustTunnel для реального подключения

### Сборка

```powershell
dotnet restore --configfile .\NuGet.Config
dotnet build .\TrustTunnelGuiClient.sln --no-restore
dotnet run --project .\src\TrustTunnel.Desktop\TrustTunnel.Desktop.csproj --no-build
dotnet format .\TrustTunnelGuiClient.sln --verify-no-changes --no-restore
```

### Где лежит desktop binary

```text
ProxyOpenHub.exe
```

## Нативные файлы

TrustTunnel runtime можно положить в `native/bundled/win-x64` или рядом с собранным desktop executable.

Предпочтительный runtime:

- `vpn_easy.dll`
- `wintun.dll`
- `vpn_easy_service.exe`

CLI fallback:

- `trusttunnel_client.exe`
- `wintun.dll`

Планируемые future binaries:

- `sing-box.exe`
- `xray.exe`
- `hysteria.exe`
- `naive.exe`

## Интерфейс

Скриншоты в репозиторий пока не добавлены. Это надо исправить.

Первый набор скриншотов должен показать:

- expanded mode;
- compact mode;
- TOML tools;
- routing profiles;
- diagnostics;
- live metrics.

## Документация

- English README: [README.en.md](README.en.md)
- Releases: [GitHub Releases](https://github.com/AkumaHacai/Proxy-Open-Hub/releases)
- Issues: [GitHub Issues](https://github.com/AkumaHacai/Proxy-Open-Hub/issues)

## Хостинг

Ниже — партнерские ссылки. Это реферальные ссылки. Если вы переходите по ним и что-то берете, проект может получить бонус. Блок не влияет на код, roadmap или технические решения.

- AEZA — бюджетные VPS, нормальный вариант для быстрых тестов: [aeza.net](https://aeza.net/?ref=520655)
- JustHost — полезен, если нужны RU-локации, в том числе Москва и Санкт-Петербург: [justhost.ru](https://justhost.ru/?ref=286200)
- Serv.Host — еще один вариант с недорогими VPS: [serv.host](https://serv.host/?from=24960)

## Лицензия

Исходный код Proxy Open Hub распространяется по Apache License 2.0. Смотрите `LICENSE.txt`.

Нативные ядра и внешние бинарники живут по своим лицензиям. Смотрите `NOTICE.md` и файлы лицензий рядом с соответствующими бинарниками.

## Дисклеймер

Proxy Open Hub — независимый проект. Он не аффилирован с TrustTunnel, sing-box, Xray-core, Hysteria2, NaiveProxy или Wintun.
