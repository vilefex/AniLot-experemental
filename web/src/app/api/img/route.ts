import type { NextRequest } from "next/server";
import { BROWSER_UA, safeUrl } from "@/lib/server/net";

// Прокси картинок. Режимы:
//   m=proxy — сервер сам скачивает картинку (https-апгрейд включён);
//   m=alt   — «альтернативное подключение»: зеркала CDN и внешние ресайзеры.
export const dynamic = "force-dynamic";

const MIRROR_DOMAINS = ["anixmirai.com", "anixsekai.com"];

function candidates(u: URL, mode: string): string[] {
  const list: string[] = [u.href];
  if (mode !== "alt") return list;
  const out: string[] = [];
  for (const d of MIRROR_DOMAINS) {
    if (u.hostname === d || u.hostname.endsWith(`.${d}`)) {
      for (const other of MIRROR_DOMAINS) {
        if (other === d) continue;
        const m = new URL(u.href);
        m.hostname = u.hostname.slice(0, u.hostname.length - d.length) + other;
        out.push(m.href);
      }
    }
  }
  const bare = `${u.hostname}${u.pathname}${u.search}`;
  out.push(`https://wsrv.nl/?url=${encodeURIComponent(bare)}&output=jpg&q=85`);
  out.push(`https://images.weserv.nl/?url=${encodeURIComponent(bare)}&output=jpg&q=85`);
  return [...list, ...out];
}

export async function GET(req: NextRequest): Promise<Response> {
  const u = safeUrl(req.nextUrl.searchParams.get("u"), true);
  if (!u) return new Response("bad url", { status: 400 });
  const mode = req.nextUrl.searchParams.get("m") === "alt" ? "alt" : "proxy";

  for (const url of candidates(u, mode)) {
    try {
      const res = await fetch(url, {
        headers: { "User-Agent": BROWSER_UA, Accept: "image/avif,image/webp,image/*,*/*;q=0.8" },
        redirect: "follow",
        signal: AbortSignal.timeout(10000),
      });
      const ct = res.headers.get("content-type") || "";
      if (!res.ok || !ct.startsWith("image/")) continue;
      const buf = await res.arrayBuffer();
      if (buf.byteLength === 0 || buf.byteLength > 15 * 1024 * 1024) continue;
      return new Response(buf, {
        headers: {
          "content-type": ct,
          "cache-control": "public, max-age=604800, stale-while-revalidate=86400, immutable",
          "x-img-source": new URL(url).hostname,
        },
      });
    } catch {
      // пробуем следующий вариант
    }
  }
  return new Response("not found", { status: 404, headers: { "cache-control": "no-store" } });
}
