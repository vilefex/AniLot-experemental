"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { ImageOff } from "lucide-react";
import { useSettings } from "@/lib/client";

/**
 * Загрузка картинок с каскадом стратегий:
 *   1. direct — напрямую, адрес http:// принудительно апгрейдится до https://
 *   2. proxy  — через наш сервер (/api/img)
 *   3. alt    — «альтернативное подключение»: зеркала CDN и внешние ресайзеры
 * Успешная/неудачная стратегия запоминается по хосту, чтобы не повторять заведомо провальные попытки.
 */

type Stage = "direct" | "proxy" | "alt";

const hostStage = new Map<string, number>(); // host -> индекс стартовой стадии в цепочке "auto"
const DIRECT_TIMEOUT = 7000;

export function upgradeHttps(url: string): string {
  if (!url) return url;
  if (url.startsWith("//")) return `https:${url}`;
  return url.replace(/^http:\/\//i, "https://");
}

function hostOf(url: string): string {
  try {
    return new URL(url).hostname;
  } catch {
    return "";
  }
}

export function Poster({
  src,
  alt = "",
  className = "",
  ratio = "2/3",
  priority = false,
}: {
  src?: string | null;
  alt?: string;
  className?: string;
  ratio?: string;
  priority?: boolean;
}) {
  const [settings] = useSettings();
  const url = useMemo(() => upgradeHttps(src || ""), [src]);
  const host = hostOf(url);

  const chain: Stage[] = useMemo(() => {
    switch (settings.img) {
      case "direct":
        return ["direct"];
      case "proxy":
        return ["proxy", "alt"];
      case "alt":
        return ["alt"];
      default:
        return ["direct", "proxy", "alt"];
    }
  }, [settings.img]);

  const startIdx = settings.img === "auto" ? Math.min(hostStage.get(host) ?? 0, chain.length - 1) : 0;
  const [state, setState] = useState<{ key: string; idx: number; loaded: boolean; failed: boolean }>({
    key: "",
    idx: startIdx,
    loaded: false,
    failed: false,
  });
  const key = `${url}|${chain.join()}`;
  const cur = state.key === key ? state : { key, idx: startIdx, loaded: false, failed: false };
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const stage = chain[cur.idx];
  const imgSrc = !url
    ? ""
    : stage === "direct"
      ? url
      : `/api/img?u=${encodeURIComponent(url)}&m=${stage === "alt" ? "alt" : "proxy"}`;

  const next = () => {
    setState((p) => {
      const base = p.key === key ? p : { key, idx: startIdx, loaded: false, failed: false };
      if (base.idx + 1 < chain.length) {
        if (settings.img === "auto" && host) hostStage.set(host, base.idx + 1);
        return { ...base, idx: base.idx + 1 };
      }
      return { ...base, failed: true };
    });
  };

  useEffect(() => {
    if (timer.current) clearTimeout(timer.current);
    if (!imgSrc || cur.loaded || cur.failed) return;
    // Медленное прямое соединение считаем неудачей и переключаемся на следующую стратегию.
    if (stage === "direct" && chain.length > 1) timer.current = setTimeout(next, DIRECT_TIMEOUT);
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [imgSrc, cur.loaded, cur.failed, stage]);

  return (
    <div className={`relative overflow-hidden bg-[#1a1a1e] ${className}`} style={{ aspectRatio: ratio }}>
      {!cur.loaded && !cur.failed && <div className="skeleton absolute inset-0" />}
      {(cur.failed || !url) && (
        <div className="absolute inset-0 grid place-items-center text-white/25">
          <ImageOff size={28} />
        </div>
      )}
      {url && !cur.failed && (
        // eslint-disable-next-line @next/next/no-img-element
        <img
          key={imgSrc}
          src={imgSrc}
          alt={alt}
          loading={priority ? "eager" : "lazy"}
          decoding="async"
          referrerPolicy="no-referrer"
          onLoad={() => {
            if (timer.current) clearTimeout(timer.current);
            if (settings.img === "auto" && host) hostStage.set(host, cur.idx);
            setState({ key, idx: cur.idx, loaded: true, failed: false });
          }}
          onError={next}
          className={`absolute inset-0 h-full w-full object-cover transition-opacity duration-300 ${cur.loaded ? "opacity-100" : "opacity-0"}`}
        />
      )}
    </div>
  );
}
