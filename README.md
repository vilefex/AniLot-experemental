# AniLot (Rust + Iced)

Неофициальный клиент Anixart, переписанный с нуля на **Rust** и **Iced** (оригинал — Objective-C/UIKit, iOS).
Один код — Linux, Windows, macOS, Android и iOS.

## Что перенесено из оригинала (v1–v10)

| Область | Реализация |
| --- | --- |
| Anixart | `src/api.rs` — собственный REST-клиент вместо `libanixart.xcframework`: вход/регистрация/восстановление, релизы, фильтр, поиск, закладки, история, избранное, оценки, комментарии и ответы, коллекции, профили, друзья, расписание, лента, настройки профиля и безопасности, уведомления |
| Плеер | `src/player.rs`, `src/hls.rs` — выбор озвучки/источника/серии, качество HLS, скорость, запоминание источника и позиции, следующая серия, внешние плееры (mpv, VLC, Outplayer, nPlayer), ссылка для «поделиться» |
| Субтитры | `src/subtitles.rs` — WebVTT, SubRip, ASS/SSA; обнаружение `<track>` и PlayerJS-описаний; выбор и запоминание дорожки |
| AniSkip | `src/aniskip.rs` — сопоставление MAL ID через Shikimori (точные названия + год/число серий, неоднозначность отбрасывается), кэш меток (14 дней / 1 день для промахов) и MAL ID (7 дней для «не найдено»); авто-пропуск в mpv через Lua-скрипт |
| Shikimori | `src/shikimori.rs` — импорт публичного списка по нику, экспорт через OAuth2, перенос прогресса, кэш соответствий, отчёт, остановка |
| Загрузки | `src/hls.rs` — mp4 и HLS (склейка сегментов), группировка по аниме |
| Обновления | `src/update.rs` — последний GitHub Release, сравнение номера сборки, ссылка на файл для текущей платформы |
| Интерфейс | `src/views*.rs` — тёмная/светлая тема, акцентный цвет (по умолчанию чёрный + оранжевый), галерея, видео-раздел |

## Ограничения

* В Iced нет видеовиджета, поэтому изображение показывает внешний плеер. На десктопе используется **mpv** (с авто-пропуском
  OP/ED, главами AniSkip и сохранением позиции), запасной вариант — VLC/системный плеер. На iOS/Android — VLC, Outplayer,
  nPlayer или системный выбор приложения. Системный «картинка в картинке» зависит от выбранного плеера.
* Видео из встроенных веб-плееров с шифрованием/JS-защитой не разбирается — для них доступно открытие в браузере.
* Зашифрованные HLS-потоки (AES-128) не загружаются.
* Shikimori OAuth использует `urn:ietf:wg:oauth:2.0:oob`: после входа на сайте код вставляется в приложение.
* Адрес API по умолчанию — `https://api-s.anixsekai.com` (альтернативный и собственный адреса задаются в «Подключении»).
* Сборки для macOS/iOS/Android проверяются только в GitHub Actions.

## Сборка

```bash
cd anilot
cargo run --release            # десктоп
cargo test --lib               # тесты
```

Для Linux нужны `libwayland-dev libxkbcommon-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev libvulkan-dev`.

Android: `cargo ndk -t arm64-v8a --platform 26 -o android/app/src/main/jniLibs build --lib --release`, затем
`gradle assembleRelease` в `android/` (иконки: `python scripts/make_icons.py android android/app/src/main/res`).
Android-часть использует форк Iced и `IcedActivity` из [iced_android](https://github.com/Santuzius/iced_android).

iOS: `cargo build --release --target aarch64-apple-ios --bin anilot`, дальше упаковка в `.app` → `Payload/` → `.ipa`
(см. workflow). IPA **не подписан**.

## Workflow

`.github/workflows/anilot.yml` собирает и публикует в Release `anilot-<major>`:

* `AniLot-linux-x86_64.tar.gz` / `.tar.zst` — внутри ELF `anilot`
* `AniLot-windows-x86_64.exe`
* `AniLot-macos-universal.dmg`
* `AniLot-android.apk`
* `AniLot.ipa` (unsigned)

Номер сборки — мажорная часть версии в `Cargo.toml` (`10.0.0` → `anilot-10`).
