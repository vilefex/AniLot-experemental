// Определение прямого видеопотока по ссылке серии Anixart.
// Поддерживается: Kodik (расшифровка /ftor), Sibnet, прямые m3u8/mp4, разбор HTML-страниц плееров.

import { BROWSER_UA, safeUrl } from "./net";

export type ResolvedSource = {
  label: string;
  height: number;
  /** Адрес через /api/stream (с нужным Referer) */
  url: string;
  /** Исходный адрес потока — для внешнего плеера (mpv/VLC) */
  raw: string;
  hls: boolean;
};

export type Resolved = {
  type: "stream" | "embed";
  provider: string;
  referer: string;
  sources: ResolvedSource[];
  subtitles: { label: string; url: string; lang: string }[];
  embedUrl?: string;
  note?: string;
};

const stream = (abs: string, referer: string) => `/api/stream?u=${encodeURIComponent(abs)}&r=${encodeURIComponent(referer)}`;

function mkSource(raw: string, referer: string, height = 0, label?: string): ResolvedSource {
  const hls = /\.m3u8(\?|$)|:hls:manifest/i.test(raw);
  return { label: label || (height ? `${height}p` : hls ? "Авто" : "Оригинал"), height, raw, hls, url: stream(raw, referer) };
}

async function getText(url: string, referer: string): Promise<string> {
  const res = await fetch(url, {
    headers: { "User-Agent": BROWSER_UA, Referer: referer, Accept: "text/html,*/*" },
    redirect: "follow",
    signal: AbortSignal.timeout(15000),
  });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.text();
}

// ───────────── Kodik ─────────────

const ftorCache = new Map<string, string>();

function rot(s: string, n: number): string {
  return s.replace(/[a-zA-Z]/g, (c) => {
    const base = c <= "Z" ? 65 : 97;
    return String.fromCharCode(((c.charCodeAt(0) - base + n) % 26) + base);
  });
}

function decodeKodik(src: string): string | null {
  if (/^(https?:)?\/\//.test(src)) return src.startsWith("//") ? `https:${src}` : src;
  for (let n = 0; n < 26; n++) {
    try {
      let b64 = rot(src, n);
      b64 += "=".repeat((4 - (b64.length % 4)) % 4);
      const d = Buffer.from(b64, "base64").toString("utf8");
      if (/^(https?:)?\/\/[\w.-]+\//.test(d)) return d.startsWith("//") ? `https:${d}` : d;
    } catch {
      // следующий сдвиг
    }
  }
  return null;
}

async function resolveKodik(pageUrl: URL): Promise<Resolved> {
  const origin = pageUrl.origin;
  const clean = `${origin}${pageUrl.pathname}`;
  const html = await getText(clean, "https://anixart.app/");

  const upRaw = html.match(/urlParams\s*=\s*'([^']*)'/)?.[1];
  const info = (k: string) => html.match(new RegExp(`vInfo\\.${k}\\s*=\\s*'([^']*)'`))?.[1] || "";
  if (!upRaw) throw new Error("Kodik: не найдены параметры плеера");
  const up = JSON.parse(upRaw) as Record<string, string>;

  let ftor = ftorCache.get(origin) || "/ftor";
  if (!ftorCache.has(origin)) {
    try {
      const js = html.match(/src="(\/assets\/js\/app\.player_[^"]+\.js)"/)?.[1];
      if (js) {
        const code = await getText(origin + js, clean);
        const b = code.match(/type:"POST",url:atob\("([^"]+)"\)/)?.[1];
        if (b) ftor = Buffer.from(b, "base64").toString("utf8");
      }
    } catch {
      // оставляем значение по умолчанию
    }
    ftorCache.set(origin, ftor);
  }

  const body = new URLSearchParams({
    d: up.d || "",
    d_sign: up.d_sign || "",
    pd: up.pd || "",
    pd_sign: up.pd_sign || "",
    ref: decodeURIComponent(up.ref || ""),
    ref_sign: up.ref_sign || "",
    bad_user: "false",
    cdn_is_working: "true",
    type: info("type"),
    hash: info("hash"),
    id: info("id"),
  });
  const res = await fetch(origin + ftor, {
    method: "POST",
    body,
    headers: {
      "User-Agent": BROWSER_UA,
      Referer: clean,
      Origin: origin,
      "X-Requested-With": "XMLHttpRequest",
      "Content-Type": "application/x-www-form-urlencoded; charset=UTF-8",
    },
    signal: AbortSignal.timeout(15000),
  });
  if (!res.ok) throw new Error(`Kodik: HTTP ${res.status}`);
  const data = (await res.json()) as { links?: Record<string, { src: string }[]> };
  const sources: ResolvedSource[] = [];
  for (const [q, arr] of Object.entries(data.links || {})) {
    const first = arr?.[0]?.src;
    const dec = first ? decodeKodik(first) : null;
    if (dec) sources.push(mkSource(dec, `${origin}/`, Number(q) || 0));
  }
  if (!sources.length) throw new Error("Kodik: поток не найден");
  sources.sort((a, b) => b.height - a.height);
  return { type: "stream", provider: "Kodik", referer: `${origin}/`, sources, subtitles: [] };
}

// ───────────── Sibnet ─────────────

async function resolveSibnet(pageUrl: URL): Promise<Resolved> {
  const referer = pageUrl.href;
  const html = await getText(pageUrl.href, "https://video.sibnet.ru/");
  const m = html.match(/src\s*:\s*["'](\/v\/[^"']+\.mp4)["']/) || html.match(/["'](\/v\/[^"']+\.mp4)["']/);
  if (!m) throw new Error("Sibnet: поток не найден");
  const raw = new URL(m[1], "https://video.sibnet.ru").href;
  return { type: "stream", provider: "Sibnet", referer, sources: [mkSource(raw, referer)], subtitles: [] };
}

// ───────────── общий разбор ─────────────

const unescapeUrl = (s: string) => s.replace(/\\\//g, "/").replace(/\\u0026/g, "&").replace(/&amp;/g, "&");

async function resolveGeneric(pageUrl: URL): Promise<Resolved> {
  const html = await getText(pageUrl.href, pageUrl.href);
  const re = /https?:(?:\\?\/){2}[^"'\s<>\\]+?\.(m3u8|mp4)[^"'\s<>\\]*/gi;
  const found: string[] = [];
  for (const m of html.matchAll(re)) found.push(unescapeUrl(m[0]));
  const uniq = [...new Set(found)].filter((u) => safeUrl(u));
  if (!uniq.length) {
    return { type: "embed", provider: pageUrl.hostname, referer: pageUrl.href, sources: [], subtitles: [], embedUrl: pageUrl.href };
  }
  uniq.sort((a, b) => Number(/\.m3u8/i.test(b)) - Number(/\.m3u8/i.test(a)));
  const subs: Resolved["subtitles"] = [];
  for (const m of html.matchAll(/https?:(?:\\?\/){2}[^"'\s<>\\]+?\.(vtt|srt)[^"'\s<>\\]*/gi)) {
    subs.push({ label: `Субтитры ${subs.length + 1}`, url: stream(unescapeUrl(m[0]), pageUrl.href), lang: "ru" });
  }
  return {
    type: "stream",
    provider: pageUrl.hostname,
    referer: pageUrl.href,
    sources: uniq.slice(0, 4).map((u, i) => mkSource(u, pageUrl.href, 0, i === 0 ? undefined : `Вариант ${i + 1}`)),
    subtitles: subs,
  };
}

export async function resolveStream(rawUrl: string): Promise<Resolved> {
  const u = safeUrl(rawUrl, true);
  if (!u) throw new Error("Некорректная ссылка серии");
  const path = u.pathname.toLowerCase();
  const host = u.hostname.toLowerCase();

  if (/\.(m3u8|mp4|webm|mkv)$/.test(path)) {
    return { type: "stream", provider: host, referer: `${u.origin}/`, sources: [mkSource(u.href, `${u.origin}/`)], subtitles: [] };
  }
  try {
    if (/kodik|aniqit|kodikplayer|kodikapi/.test(host)) return await resolveKodik(u);
    if (host.endsWith("sibnet.ru")) return await resolveSibnet(u);
    return await resolveGeneric(u);
  } catch (e) {
    return {
      type: "embed",
      provider: host,
      referer: u.href,
      sources: [],
      subtitles: [],
      embedUrl: u.href,
      note: e instanceof Error ? e.message : String(e),
    };
  }
}
