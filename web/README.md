# AniLot Web

Веб-версия AniLot (Next.js) со встроенным плеером. Интерфейс — по мотивам [AniLibrix](https://github.com/pavloniym/anilibrix),
цвета AniLot (чёрный + оранжевый `#FF8A00`).

```bash
cd web
npm install
npm run dev      # http://localhost:3000
npm run build && npm start
```

## Что внутри

| Часть | Файлы |
| --- | --- |
| Собственный плеер (hls.js, горячие клавиши как в mpv, AniSkip, выбор качества/скорости, PiP, автопереход, запоминание позиции) | `src/components/Player.tsx` |
| Экспорт во внешний плеер: команда **mpv**, `.m3u` для mpv/VLC, ссылка потока | меню «⇱» в плеере |
| Определение потока: Kodik (расшифровка `/ftor`), Sibnet, прямые m3u8/mp4, разбор HTML; запасной вариант — embed | `src/lib/server/resolve.ts`, `/api/resolve` |
| Медиа-прокси (Referer, CORS, переписывание HLS, Range) | `/api/stream` |
| Картинки: **https-апгрейд → прокси-fallback → альтернативное подключение** | `src/components/Poster.tsx`, `/api/img` |
| Anixart API с переключением на запасной адрес | `/api/ax/[...path]` |
| AniSkip через Shikimori | `/api/skip` |

## Горячие клавиши плеера

`Space/K` пауза · `←/→` ±5 с (`Shift` ±30 с) · `J/L` ±10 с · `↑/↓` громкость · `M` звук · `F` полный экран · `P` PiP ·
`[` `]` скорость · `Backspace` ×1 · `S` пропуск OP/ED · `N/B` следующая/предыдущая серия · `Esc` назад.

## Картинки

1. **direct** — прямой запрос, `http://` принудительно заменяется на `https://`;
2. **proxy** — `/api/img?m=proxy`: сервер сам скачивает файл;
3. **alt** — `/api/img?m=alt`: зеркала CDN (`anixmirai.com` ⇄ `anixsekai.com`), затем `wsrv.nl` / `images.weserv.nl`.

Рабочий способ запоминается для каждого хоста; медленное прямое соединение (7 с) считается неудачей. Режим можно
зафиксировать в «Настройки → Картинки».

## Ограничения

* Некоторые источники (например, Sibnet) блокируют запросы с IP дата-центров — тогда плеер открывает встроенный веб-плеер источника.
* Токен Anixart хранится только в `localStorage` браузера.
