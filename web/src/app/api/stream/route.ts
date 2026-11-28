import type { NextRequest } from "next/server";
import { BROWSER_UA, safeUrl } from "@/lib/server/net";

// Медиа-прокси: добавляет Referer, обходит CORS, переписывает HLS-плейлисты и поддерживает Range.
export const dynamic = "force-dynamic";
export const maxDuration = 60;

function proxied(abs: string, referer: string): string {
  return `/api/stream?u=${encodeURIComponent(abs)}&r=${encodeURIComponent(referer)}`;
}

function rewritePlaylist(text: string, base: URL, referer: string): string {
  const abs = (ref: string) => {
    try {
      return new URL(ref, base).href;
    } catch {
      return ref;
    }
  };
  return text
    .split(/\r?\n/)
    .map((line) => {
      const t = line.trim();
      if (!t) return line;
      if (t.startsWith("#")) {
        return line.replace(/URI="([^"]+)"/g, (_m, uri: string) => `URI="${proxied(abs(uri), referer)}"`);
      }
      return proxied(abs(t), referer);
    })
    .join("\n");
}

export async function GET(req: NextRequest): Promise<Response> {
  const u = safeUrl(req.nextUrl.searchParams.get("u"), true);
  if (!u) return new Response("bad url", { status: 400 });
  const referer = req.nextUrl.searchParams.get("r") || `${u.origin}/`;

  const headers: Record<string, string> = { "User-Agent": BROWSER_UA, Referer: referer, Accept: "*/*" };
  try {
    headers.Origin = new URL(referer).origin;
  } catch {}
  const range = req.headers.get("range");
  if (range) headers.Range = range;

  let upstream: Response;
  try {
    upstream = await fetch(u.href, { headers, redirect: "follow", signal: AbortSignal.timeout(30000) });
  } catch (e) {
    return new Response(`upstream error: ${e instanceof Error ? e.message : e}`, { status: 502 });
  }

  // После редиректов относительные ссылки плейлиста считаются от финального адреса.
  const finalUrl = safeUrl(upstream.url) || u;
  const ct = upstream.headers.get("content-type") || "";
  const path = finalUrl.pathname.toLowerCase();
  const isPlaylist = path.endsWith(".m3u8") || /mpegurl/i.test(ct);

  if (isPlaylist && upstream.ok) {
    const text = await upstream.text();
    if (text.trimStart().startsWith("#EXTM3U")) {
      return new Response(rewritePlaylist(text, finalUrl, referer), {
        headers: { "content-type": "application/vnd.apple.mpegurl", "cache-control": "no-store" },
      });
    }
    return new Response(text, { status: upstream.status, headers: { "content-type": ct || "text/plain" } });
  }

  const out = new Headers();
  for (const h of ["content-type", "content-length", "content-range", "accept-ranges", "etag", "last-modified"]) {
    const v = upstream.headers.get(h);
    if (v) out.set(h, v);
  }
  if (!out.has("accept-ranges")) out.set("accept-ranges", "bytes");
  out.set("cache-control", path.endsWith(".ts") || path.endsWith(".m4s") ? "public, max-age=3600" : "no-store");
  return new Response(upstream.body, { status: upstream.status, headers: out });
}
