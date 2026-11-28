"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type Hls from "hls.js";
import {
  ArrowLeft, ChevronsRight, Download, ExternalLink, Gauge, ListVideo, Loader2, Maximize, Minimize, Pause, PictureInPicture2,
  Play, RotateCcw, RotateCw, Settings2, SkipBack, SkipForward, Subtitles, Terminal, Volume1, Volume2, VolumeX,
} from "lucide-react";
import { fmtTime, useSettings } from "@/lib/client";

type Resolved = {
  type: "stream" | "embed";
  provider: string;
  referer: string;
  sources: { label: string; height: number; url: string; raw: string; hls: boolean }[];
  subtitles: { label: string; url: string; lang: string }[];
  embedUrl?: string;
  note?: string;
};
type Marker = { type: string; start: number; end: number };

export type PlayerEpisode = { position: number; name: string };

export type PlayerProps = {
  releaseId: number;
  title: string;
  titleOriginal: string;
  year: number;
  episodeName: string;
  episodeNumber: number;
  voiceName: string;
  sourceName: string;
  url: string;
  episodes: PlayerEpisode[];
  currentPosition: number;
  startAt: number;
  onBack: () => void;
  onSelect: (position: number) => void;
  onProgress: (time: number, duration: number) => void;
  onWatched: () => void;
};

const SPEEDS = [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];
const MARKER_NAMES: Record<string, string> = { op: "опенинг", ed: "эндинг", recap: "рекап" };

export default function Player(p: PlayerProps) {
  const [settings, setSettings] = useSettings();
  const box = useRef<HTMLDivElement>(null);
  const video = useRef<HTMLVideoElement>(null);
  const hls = useRef<Hls | null>(null);
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const seekTo = useRef<number>(p.startAt);
  const watchedSent = useRef(false);
  const skipped = useRef<Set<string>>(new Set());
  const lastToast = useRef<ReturnType<typeof setTimeout> | null>(null);

  const [res, setRes] = useState<Resolved | null>(null);
  const [resolving, setResolving] = useState(true);
  const [err, setErr] = useState("");
  const [srcIdx, setSrcIdx] = useState(0);
  const [subIdx, setSubIdx] = useState(-1);
  const [markers, setMarkers] = useState<Marker[]>([]);

  const [paused, setPaused] = useState(true);
  const [time, setTime] = useState(0);
  const [dur, setDur] = useState(0);
  const [buffEnd, setBuffEnd] = useState(0);
  const [muted, setMuted] = useState(false);
  const [speed, setSpeedState] = useState(settings.speed || 1);
  const [full, setFull] = useState(false);
  const [ui, setUi] = useState(true);
  const [waiting, setWaiting] = useState(true);
  const [menu, setMenu] = useState<null | "speed" | "quality" | "ext" | "eps" | "subs">(null);
  const [toast, setToast] = useState("");
  const [hover, setHover] = useState<{ x: number; t: number } | null>(null);
  const [retry, setRetry] = useState(0);

  const idx = p.episodes.findIndex((e) => e.position === p.currentPosition);
  const prevEp = idx > 0 ? p.episodes[idx - 1] : null;
  const nextEp = idx >= 0 && idx < p.episodes.length - 1 ? p.episodes[idx + 1] : null;

  const say = useCallback((t: string) => {
    setToast(t);
    if (lastToast.current) clearTimeout(lastToast.current);
    lastToast.current = setTimeout(() => setToast(""), 1400);
  }, []);

  // ───── определение потока ─────
  useEffect(() => {
    const ac = new AbortController();
    setResolving(true);
    setErr("");
    setRes(null);
    setSrcIdx(0);
    setSubIdx(-1);
    setTime(0);
    setDur(0);
    setBuffEnd(0);
    seekTo.current = p.startAt;
    watchedSent.current = false;
    skipped.current = new Set();
    fetch(`/api/resolve?url=${encodeURIComponent(p.url)}`, { signal: ac.signal })
      .then((r) => r.json())
      .then((d: Resolved & { error?: string }) => {
        if (d.error) throw new Error(d.error);
        setRes(d);
        if (d.type === "stream" && d.sources.length) {
          const want = settings.quality;
          let best = 0;
          if (want > 0) {
            const ok = d.sources.map((s, i) => ({ s, i })).filter((x) => x.s.height && x.s.height <= want);
            if (ok.length) best = ok.sort((a, b) => b.s.height - a.s.height)[0].i;
            else best = d.sources.length - 1;
          }
          setSrcIdx(best);
        }
      })
      .catch((e) => {
        if (!ac.signal.aborted) setErr(e instanceof Error ? e.message : "Не удалось определить поток");
      })
      .finally(() => !ac.signal.aborted && setResolving(false));
    return () => ac.abort();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [p.url, retry]);

  // ───── метки AniSkip ─────
  useEffect(() => {
    setMarkers([]);
    if (!p.episodeNumber || !p.titleOriginal) return;
    const ac = new AbortController();
    const q = new URLSearchParams({ title: p.titleOriginal, ru: p.title, year: String(p.year || ""), ep: String(p.episodeNumber) });
    fetch(`/api/skip?${q}`, { signal: ac.signal })
      .then((r) => r.json())
      .then((d: { markers?: Marker[] }) => setMarkers(d.markers || []))
      .catch(() => {});
    return () => ac.abort();
  }, [p.releaseId, p.episodeNumber, p.titleOriginal, p.title, p.year]);

  // ───── подключение источника ─────
  const source = res?.type === "stream" ? res.sources[srcIdx] : undefined;
  useEffect(() => {
    const v = video.current;
    if (!v || !source) return;
    let cancelled = false;
    let netRetries = 0;
    setWaiting(true);
    setErr("");

    const begin = () => {
      const t = seekTo.current;
      if (t > 1) {
        try {
          v.currentTime = t;
        } catch {}
      }
      seekTo.current = 0;
      v.playbackRate = speed;
      v.play().catch(() => setPaused(true));
    };

    (async () => {
      if (source.hls) {
        const { default: HlsLib } = await import("hls.js");
        if (cancelled) return;
        if (HlsLib.isSupported()) {
          const h = new HlsLib({ enableWorker: true, maxBufferLength: 60, startPosition: seekTo.current > 1 ? seekTo.current : -1 });
          hls.current = h;
          h.loadSource(source.url);
          h.attachMedia(v);
          h.on(HlsLib.Events.MANIFEST_PARSED, () => {
            seekTo.current = 0;
            v.playbackRate = speed;
            v.play().catch(() => setPaused(true));
          });
          h.on(HlsLib.Events.ERROR, (_e, data) => {
            if (!data.fatal) return;
            if (data.type === HlsLib.ErrorTypes.NETWORK_ERROR && netRetries++ < 3) h.startLoad();
            else if (data.type === HlsLib.ErrorTypes.MEDIA_ERROR) h.recoverMediaError();
            else setErr("Ошибка воспроизведения потока. Попробуйте другое качество или источник.");
          });
        } else if (v.canPlayType("application/vnd.apple.mpegurl")) {
          v.src = source.url;
          v.addEventListener("loadedmetadata", begin, { once: true });
        } else setErr("Браузер не поддерживает HLS");
      } else {
        v.src = source.url;
        v.addEventListener("loadedmetadata", begin, { once: true });
      }
    })();

    return () => {
      cancelled = true;
      if (hls.current) {
        hls.current.destroy();
        hls.current = null;
      }
      v.removeEventListener("loadedmetadata", begin);
      v.removeAttribute("src");
      v.load();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [source?.url]);

  // ───── отчёт о прогрессе ─────
  const timeRef = useRef({ t: 0, d: 0 });
  timeRef.current = { t: time, d: dur };
  useEffect(() => {
    const id = setInterval(() => {
      const v = video.current;
      if (v && !v.paused && v.duration) p.onProgress(v.currentTime, v.duration);
    }, 5000);
    return () => {
      clearInterval(id);
      const { t, d } = timeRef.current;
      if (d > 0 && t > 1) p.onProgress(t, d);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [p.currentPosition]);

  // ───── управление ─────
  const toggle = useCallback(() => {
    const v = video.current;
    if (!v) return;
    if (v.paused) v.play().catch(() => {});
    else v.pause();
  }, []);

  const seek = useCallback(
    (delta: number) => {
      const v = video.current;
      if (!v || !v.duration) return;
      v.currentTime = Math.min(Math.max(0, v.currentTime + delta), v.duration);
      say(`${delta > 0 ? "+" : "−"}${Math.abs(delta)} с`);
    },
    [say],
  );

  const setSpeed = useCallback(
    (s: number) => {
      const v = video.current;
      const val = Math.min(2.5, Math.max(0.25, Math.round(s * 100) / 100));
      if (v) v.playbackRate = val;
      setSpeedState(val);
      setSettings({ speed: val });
      say(`Скорость ×${val}`);
    },
    [say, setSettings],
  );

  const setVolume = useCallback(
    (val: number) => {
      const v = video.current;
      const x = Math.min(1, Math.max(0, val));
      if (v) {
        v.volume = x;
        v.muted = x === 0;
      }
      setSettings({ volume: x });
      say(`Громкость ${Math.round(x * 100)}%`);
    },
    [say, setSettings],
  );

  const toggleFull = useCallback(() => {
    const el = box.current;
    if (!el) return;
    if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
    else el.requestFullscreen?.().catch(() => {});
  }, []);

  const pip = useCallback(async () => {
    const v = video.current;
    if (!v) return;
    try {
      if (document.pictureInPictureElement) await document.exitPictureInPicture();
      else await v.requestPictureInPicture();
    } catch {
      say("PiP недоступен");
    }
  }, [say]);

  const activeMarker = useMemo(
    () => markers.find((m) => time >= m.start && time < m.end - 1),
    [markers, time],
  );
  const skipNow = useCallback(() => {
    const v = video.current;
    if (!v || !activeMarker) return;
    v.currentTime = activeMarker.end;
    say(`Пропуск: ${MARKER_NAMES[activeMarker.type] || activeMarker.type}`);
  }, [activeMarker, say]);

  // авто-пропуск
  useEffect(() => {
    if (!settings.autoSkip || !activeMarker) return;
    const k = `${p.currentPosition}:${activeMarker.type}:${activeMarker.start}`;
    if (skipped.current.has(k)) return;
    skipped.current.add(k);
    skipNow();
  }, [activeMarker, settings.autoSkip, p.currentPosition, skipNow]);

  // громкость при старте
  useEffect(() => {
    const v = video.current;
    if (v) v.volume = settings.volume;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // автоскрытие интерфейса
  const poke = useCallback(() => {
    setUi(true);
    if (hideTimer.current) clearTimeout(hideTimer.current);
    hideTimer.current = setTimeout(() => setUi(false), 3200);
  }, []);
  useEffect(() => {
    poke();
    return () => {
      if (hideTimer.current) clearTimeout(hideTimer.current);
    };
  }, [poke]);

  useEffect(() => {
    const onFs = () => setFull(!!document.fullscreenElement);
    document.addEventListener("fullscreenchange", onFs);
    return () => document.removeEventListener("fullscreenchange", onFs);
  }, []);

  // горячие клавиши (как в mpv)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      const v = video.current;
      let handled = true;
      switch (e.key) {
        case " ":
        case "k":
        case "K":
          toggle();
          break;
        case "ArrowLeft":
          seek(e.shiftKey ? -30 : -5);
          break;
        case "ArrowRight":
          seek(e.shiftKey ? 30 : 5);
          break;
        case "j":
        case "J":
          seek(-10);
          break;
        case "l":
        case "L":
          seek(10);
          break;
        case "ArrowUp":
          setVolume((v?.volume ?? 1) + 0.05);
          break;
        case "ArrowDown":
          setVolume((v?.volume ?? 1) - 0.05);
          break;
        case "m":
        case "M":
          if (v) {
            v.muted = !v.muted;
            setMuted(v.muted);
            say(v.muted ? "Звук выключен" : "Звук включён");
          }
          break;
        case "f":
        case "F":
          toggleFull();
          break;
        case "p":
        case "P":
          pip();
          break;
        case "[":
          setSpeed(speed - 0.25);
          break;
        case "]":
          setSpeed(speed + 0.25);
          break;
        case "Backspace":
          setSpeed(1);
          break;
        case "s":
        case "S":
          skipNow();
          break;
        case "n":
        case "N":
        case ">":
          if (nextEp) p.onSelect(nextEp.position);
          break;
        case "b":
        case "B":
        case "<":
          if (prevEp) p.onSelect(prevEp.position);
          break;
        case "0":
        case "Home":
          if (v) v.currentTime = 0;
          break;
        case "Escape":
          if (menu) setMenu(null);
          else if (!document.fullscreenElement) p.onBack();
          break;
        default:
          handled = false;
      }
      if (handled) {
        e.preventDefault();
        poke();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // ───── события <video> ─────
  const onTime = () => {
    const v = video.current;
    if (!v) return;
    setTime(v.currentTime);
    if (v.buffered.length) {
      let end = 0;
      for (let i = 0; i < v.buffered.length; i++) if (v.buffered.start(i) <= v.currentTime + 0.5) end = Math.max(end, v.buffered.end(i));
      setBuffEnd(end);
    }
    if (!watchedSent.current && v.duration && v.currentTime / v.duration > 0.85) {
      watchedSent.current = true;
      p.onWatched();
    }
  };

  // ───── внешние плееры ─────
  const rawUrl = source?.raw || "";
  const mediaTitle = `${p.title} — ${p.episodeName}`;
  const mpvCommand = useMemo(() => {
    const q = (s: string) => `"${s.replace(/(["\\$`])/g, "\\$1")}"`;
    return [
      "mpv",
      `--force-media-title=${q(mediaTitle)}`,
      res?.referer ? `--referrer=${q(res.referer)}` : "",
      "--user-agent=\"Mozilla/5.0\"",
      time > 5 ? `--start=${Math.floor(time)}` : "",
      q(rawUrl),
    ]
      .filter(Boolean)
      .join(" ");
  }, [mediaTitle, res?.referer, rawUrl, time]);

  const copy = async (text: string, msg: string) => {
    try {
      await navigator.clipboard.writeText(text);
      say(msg);
    } catch {
      window.prompt("Скопируйте вручную:", text);
    }
    setMenu(null);
  };

  const downloadM3u = () => {
    const lines = ["#EXTM3U", `#EXTINF:-1,${mediaTitle}`];
    if (res?.referer) lines.push(`#EXTVLCOPT:http-referrer=${res.referer}`, "#EXTVLCOPT:http-user-agent=Mozilla/5.0");
    lines.push(rawUrl);
    const blob = new Blob([lines.join("\n")], { type: "audio/x-mpegurl" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = `${mediaTitle.replace(/[^\p{L}\p{N}\-_. ]/gu, "")}.m3u`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 2000);
    setMenu(null);
  };

  // ───── визуальная часть ─────
  const pct = dur ? (time / dur) * 100 : 0;
  const buffPct = dur ? (buffEnd / dur) * 100 : 0;
  const vol = muted ? 0 : settings.volume;
  const VolIcon = vol === 0 ? VolumeX : vol < 0.5 ? Volume1 : Volume2;
  const showUi = ui || paused || menu !== null;

  const timelineMove = (e: React.MouseEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const x = Math.min(Math.max(0, e.clientX - r.left), r.width);
    setHover({ x, t: (x / r.width) * dur });
  };
  const timelineClick = (e: React.MouseEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const v = video.current;
    if (v && dur) v.currentTime = ((e.clientX - r.left) / r.width) * dur;
  };

  const menuCls =
    "absolute bottom-full right-0 mb-3 min-w-44 rounded-xl border border-white/10 bg-[#141416]/95 p-1.5 shadow-2xl backdrop-blur fade-in";
  const itemCls = (on: boolean) =>
    `flex w-full items-center justify-between gap-6 rounded-lg px-3 py-2 text-left text-sm hover:bg-white/10 ${on ? "text-accent font-semibold" : "text-white/90"}`;
  const iconBtn = "grid h-10 w-10 place-items-center rounded-full text-white/90 transition hover:bg-white/15 hover:text-white disabled:opacity-30";

  return (
    <div
      ref={box}
      onMouseMove={poke}
      onClick={() => menu && setMenu(null)}
      className={`relative h-screen w-full select-none overflow-hidden bg-black ${showUi || res?.type === "embed" ? "" : "cursor-none"}`}
    >
      {/* видео */}
      <video
        ref={video}
        className="absolute inset-0 h-full w-full bg-black object-contain"
        playsInline
        crossOrigin="anonymous"
        onClick={(e) => {
          e.stopPropagation();
          if (menu) setMenu(null);
          else toggle();
        }}
        onDoubleClick={toggleFull}
        onPlay={() => setPaused(false)}
        onPause={() => setPaused(true)}
        onTimeUpdate={onTime}
        onProgress={onTime}
        onDurationChange={(e) => setDur(e.currentTarget.duration || 0)}
        onVolumeChange={(e) => setMuted(e.currentTarget.muted)}
        onWaiting={() => setWaiting(true)}
        onPlaying={() => setWaiting(false)}
        onCanPlay={() => setWaiting(false)}
        onEnded={() => {
          if (!watchedSent.current) {
            watchedSent.current = true;
            p.onWatched();
          }
          if (settings.autoNext && nextEp) p.onSelect(nextEp.position);
        }}
        onError={() => {
          if (source) setErr("Не удалось загрузить видео. Попробуйте другое качество или источник.");
        }}
      >
        {res?.subtitles.map((s, i) => (
          <track key={s.url} kind="subtitles" label={s.label} srcLang={s.lang} src={s.url} default={i === subIdx} />
        ))}
      </video>

      {/* embed-фолбэк */}
      {res?.type === "embed" && res.embedUrl && (
        <div className="absolute inset-0 flex flex-col bg-black pt-16">
          <div className="mx-auto mb-2 max-w-3xl px-4 text-center text-xs text-white/60">
            Источник «{res.provider}» не отдаёт прямой поток — открыт встроенный веб-плеер.
            {res.note ? ` (${res.note})` : ""}
          </div>
          <iframe
            src={res.embedUrl}
            className="h-full w-full flex-1 border-0"
            allow="autoplay; fullscreen; picture-in-picture; encrypted-media"
            allowFullScreen
            referrerPolicy="origin"
          />
        </div>
      )}

      {/* загрузка / ошибка */}
      {(resolving || (res?.type === "stream" && waiting && !err && !paused) || (res?.type === "stream" && waiting && !err && time === 0)) && (
        <div className="pointer-events-none absolute inset-0 grid place-items-center">
          <div className="flex flex-col items-center gap-3 text-white/80">
            <Loader2 className="animate-spin text-accent" size={52} />
            {resolving && <span className="text-sm">Получаем видеопоток…</span>}
          </div>
        </div>
      )}
      {err && (
        <div className="absolute inset-0 grid place-items-center bg-black/70">
          <div className="max-w-md rounded-2xl border border-white/10 bg-surface p-6 text-center">
            <p className="mb-4 text-sm text-white/80">{err}</p>
            <div className="flex flex-wrap justify-center gap-2">
              <button className="btn btn-primary" onClick={() => setRetry((n) => n + 1)}>
                <RotateCcw size={16} /> Повторить
              </button>
              <button className="btn" onClick={p.onBack}>
                Назад
              </button>
            </div>
          </div>
        </div>
      )}

      {/* подсказка (громкость, скорость …) */}
      {toast && (
        <div className="pointer-events-none absolute left-1/2 top-24 -translate-x-1/2 rounded-lg bg-black/70 px-4 py-2 text-sm font-medium backdrop-blur fade-in">
          {toast}
        </div>
      )}

      {/* кнопка пропуска */}
      {activeMarker && res?.type === "stream" && (
        <button
          onClick={(e) => {
            e.stopPropagation();
            skipNow();
          }}
          className="absolute bottom-32 right-8 z-20 flex items-center gap-2 rounded-lg border border-white/20 bg-black/70 px-5 py-3 text-sm font-semibold backdrop-blur transition hover:bg-accent hover:text-black fade-in"
        >
          Пропустить {MARKER_NAMES[activeMarker.type] || "заставку"} <ChevronsRight size={18} />
        </button>
      )}

      {/* центральная кнопка при паузе */}
      {paused && !resolving && !err && res?.type === "stream" && !waiting && (
        <button
          onClick={(e) => {
            e.stopPropagation();
            toggle();
          }}
          className="absolute left-1/2 top-1/2 grid h-20 w-20 -translate-x-1/2 -translate-y-1/2 place-items-center rounded-full bg-black/50 text-white backdrop-blur transition hover:scale-105 hover:bg-accent hover:text-black fade-in"
        >
          <Play size={36} fill="currentColor" />
        </button>
      )}

      {/* шапка */}
      <div
        className={`absolute inset-x-0 top-0 z-20 flex items-center gap-3 bg-gradient-to-b from-black/85 to-transparent px-6 pb-10 pt-4 transition-opacity duration-300 ${showUi ? "opacity-100" : "pointer-events-none opacity-0"}`}
        onClick={(e) => e.stopPropagation()}
      >
        <button className={iconBtn} onClick={p.onBack} title="Назад (Esc)">
          <ArrowLeft size={22} />
        </button>
        <div className="min-w-0">
          <div className="truncate text-lg font-bold leading-tight">{p.title}</div>
          <div className="truncate text-sm text-white/70">
            {p.episodeName}
            {p.voiceName ? ` · ${p.voiceName}` : ""}
            {p.sourceName ? ` · ${p.sourceName}` : ""}
          </div>
        </div>
      </div>

      {/* нижняя панель */}
      {res?.type === "stream" && (
        <div
          className={`absolute inset-x-0 bottom-0 z-20 bg-gradient-to-t from-black/90 via-black/50 to-transparent px-6 pb-4 pt-16 transition-opacity duration-300 ${showUi ? "opacity-100" : "pointer-events-none opacity-0"}`}
          onClick={(e) => e.stopPropagation()}
        >
          {/* таймлайн */}
          <div
            className="group relative mb-2 flex h-5 cursor-pointer items-center"
            onMouseMove={timelineMove}
            onMouseLeave={() => setHover(null)}
            onClick={timelineClick}
          >
            <div className="relative h-1 w-full rounded bg-white/25 transition-all group-hover:h-1.5">
              <div className="absolute inset-y-0 left-0 rounded bg-white/30" style={{ width: `${buffPct}%` }} />
              {markers.map((m, i) =>
                dur ? (
                  <div
                    key={i}
                    className="absolute inset-y-0 bg-white/50"
                    style={{ left: `${(m.start / dur) * 100}%`, width: `${((m.end - m.start) / dur) * 100}%` }}
                    title={MARKER_NAMES[m.type] || m.type}
                  />
                ) : null,
              )}
              <div className="absolute inset-y-0 left-0 rounded bg-accent" style={{ width: `${pct}%` }} />
              <div
                className="absolute top-1/2 h-3.5 w-3.5 -translate-x-1/2 -translate-y-1/2 scale-0 rounded-full bg-accent transition group-hover:scale-100"
                style={{ left: `${pct}%` }}
              />
            </div>
            {hover && (
              <div
                className="pointer-events-none absolute -top-8 -translate-x-1/2 rounded bg-black/85 px-2 py-1 text-xs"
                style={{ left: hover.x }}
              >
                {fmtTime(hover.t)}
              </div>
            )}
          </div>

          <div className="grid grid-cols-[1fr_auto_1fr] items-center gap-4">
            {/* слева */}
            <div className="flex items-center gap-1">
              <button className={iconBtn} onClick={() => setMenu(menu === "eps" ? null : "eps")} title="Эпизоды">
                <ListVideo size={22} />
              </button>
              <span className="ml-2 text-sm tabular-nums text-white/80">
                {fmtTime(time)} / {fmtTime(dur)}
              </span>
              {markers.length > 0 && (
                <button
                  className={`ml-2 rounded-md px-2 py-1 text-xs ${settings.autoSkip ? "bg-accent/20 text-accent" : "bg-white/10 text-white/60"}`}
                  onClick={() => setSettings({ autoSkip: !settings.autoSkip })}
                  title="Автопропуск OP/ED (AniSkip)"
                >
                  Автопропуск
                </button>
              )}
            </div>

            {/* центр */}
            <div className="flex items-center gap-2">
              <button className={iconBtn} disabled={!prevEp} onClick={() => prevEp && p.onSelect(prevEp.position)} title="Предыдущая (B)">
                <SkipBack size={22} />
              </button>
              <button className={iconBtn} onClick={() => seek(-10)} title="−10 с (J)">
                <RotateCcw size={22} />
              </button>
              <button
                onClick={toggle}
                className="grid h-12 w-12 place-items-center rounded-full bg-accent text-black transition hover:scale-105"
                title="Пауза / воспроизведение (Space)"
              >
                {paused ? <Play size={24} fill="currentColor" /> : <Pause size={24} fill="currentColor" />}
              </button>
              <button className={iconBtn} onClick={() => seek(10)} title="+10 с (L)">
                <RotateCw size={22} />
              </button>
              <button className={iconBtn} disabled={!nextEp} onClick={() => nextEp && p.onSelect(nextEp.position)} title="Следующая (N)">
                <SkipForward size={22} />
              </button>
            </div>

            {/* справа */}
            <div className="flex items-center justify-end gap-1">
              <div className="group/vol flex items-center">
                <input
                  type="range"
                  min={0}
                  max={1}
                  step={0.01}
                  value={vol}
                  onChange={(e) => setVolume(Number(e.target.value))}
                  className="player-range mr-1 w-0 opacity-0 transition-all group-hover/vol:w-24 group-hover/vol:opacity-100"
                />
                <button
                  className={iconBtn}
                  onClick={() => {
                    const v = video.current;
                    if (v) v.muted = !v.muted;
                  }}
                  title="Звук (M)"
                >
                  <VolIcon size={22} />
                </button>
              </div>

              <div className="relative">
                <button className={`${iconBtn} w-auto gap-1 px-3 text-sm font-semibold`} onClick={() => setMenu(menu === "speed" ? null : "speed")} title="Скорость ([ ])">
                  <Gauge size={20} /> ×{speed}
                </button>
                {menu === "speed" && (
                  <div className={menuCls}>
                    {SPEEDS.map((s) => (
                      <button key={s} className={itemCls(s === speed)} onClick={() => { setSpeed(s); setMenu(null); }}>
                        ×{s}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {res.sources.length > 1 && (
                <div className="relative">
                  <button className={`${iconBtn} w-auto gap-1 px-3 text-sm font-semibold`} onClick={() => setMenu(menu === "quality" ? null : "quality")} title="Качество">
                    <Settings2 size={20} /> {source?.label}
                  </button>
                  {menu === "quality" && (
                    <div className={menuCls}>
                      {res.sources.map((s, i) => (
                        <button
                          key={s.url}
                          className={itemCls(i === srcIdx)}
                          onClick={() => {
                            seekTo.current = video.current?.currentTime || 0;
                            setSrcIdx(i);
                            if (s.height) setSettings({ quality: s.height });
                            setMenu(null);
                          }}
                        >
                          {s.label}
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              )}

              {res.subtitles.length > 0 && (
                <div className="relative">
                  <button className={iconBtn} onClick={() => setMenu(menu === "subs" ? null : "subs")} title="Субтитры">
                    <Subtitles size={22} />
                  </button>
                  {menu === "subs" && (
                    <div className={menuCls}>
                      <button className={itemCls(subIdx === -1)} onClick={() => { setSubIdx(-1); setMenu(null); setTimeout(() => { const t = video.current?.textTracks; if (t) for (let i = 0; i < t.length; i++) t[i].mode = "disabled"; }, 0); }}>
                        Выключены
                      </button>
                      {res.subtitles.map((s, i) => (
                        <button
                          key={s.url}
                          className={itemCls(subIdx === i)}
                          onClick={() => {
                            setSubIdx(i);
                            setMenu(null);
                            setTimeout(() => {
                              const t = video.current?.textTracks;
                              if (t) for (let k = 0; k < t.length; k++) t[k].mode = k === i ? "showing" : "disabled";
                            }, 50);
                          }}
                        >
                          {s.label}
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              )}

              <div className="relative">
                <button className={iconBtn} onClick={() => setMenu(menu === "ext" ? null : "ext")} title="Открыть во внешнем плеере (mpv / VLC)">
                  <ExternalLink size={21} />
                </button>
                {menu === "ext" && (
                  <div className={menuCls + " min-w-64"}>
                    <button className={itemCls(false)} onClick={() => copy(mpvCommand, "Команда mpv скопирована")}>
                      <span className="flex items-center gap-2"><Terminal size={16} /> Копировать команду mpv</span>
                    </button>
                    <button className={itemCls(false)} onClick={() => copy(rawUrl, "Ссылка на поток скопирована")}>
                      <span className="flex items-center gap-2"><ExternalLink size={16} /> Копировать ссылку потока</span>
                    </button>
                    <button className={itemCls(false)} onClick={downloadM3u}>
                      <span className="flex items-center gap-2"><Download size={16} /> Скачать .m3u (mpv / VLC)</span>
                    </button>
                  </div>
                )}
              </div>

              <button className={iconBtn} onClick={pip} title="Картинка в картинке (P)">
                <PictureInPicture2 size={21} />
              </button>
              <button className={iconBtn} onClick={toggleFull} title="Полный экран (F)">
                {full ? <Minimize size={22} /> : <Maximize size={22} />}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* список эпизодов */}
      {menu === "eps" && (
        <div
          className="absolute bottom-28 left-6 z-30 max-h-[60vh] w-80 overflow-y-auto rounded-xl border border-white/10 bg-[#141416]/95 p-2 shadow-2xl backdrop-blur fade-in"
          onClick={(e) => e.stopPropagation()}
        >
          {p.episodes.map((e) => (
            <button
              key={e.position}
              className={itemCls(e.position === p.currentPosition)}
              onClick={() => {
                setMenu(null);
                if (e.position !== p.currentPosition) p.onSelect(e.position);
              }}
            >
              <span className="truncate">{e.name}</span>
              {e.position === p.currentPosition && <Play size={14} fill="currentColor" />}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
