import type { NextRequest } from "next/server";
import { API_UA, json, safeUrl } from "@/lib/server/net";

// Прокси к Anixart API с автоматическим переключением на альтернативный адрес.
export const dynamic = "force-dynamic";

const PRIMARY = "https://api-s.anixsekai.com";
const ALT = "https://api.anixsekai.com";

type Ctx = { params: Promise<{ path: string[] }> };

async function handler(req: NextRequest, ctx: Ctx): Promise<Response> {
  const { path } = await ctx.params;
  const mode = req.headers.get("x-ax-mode");
  const customRaw = (req.headers.get("x-ax-custom") || "").trim();

  let bases: string[];
  if (customRaw) {
    const u = safeUrl(customRaw);
    if (!u) return json({ code: -1, error: "Некорректный собственный адрес API" }, 400);
    bases = [u.origin + u.pathname.replace(/\/$/, "")];
  } else {
    bases = mode === "alt" ? [ALT, PRIMARY] : [PRIMARY, ALT];
  }

  const hasBody = !["GET", "HEAD"].includes(req.method);
  const body = hasBody ? await req.arrayBuffer() : undefined;
  const search = req.nextUrl.search;
  const target = path.map(encodeURIComponent).join("/");

  const headers: Record<string, string> = { "User-Agent": API_UA, "API-Version": "v2", Accept: "application/json" };
  const ct = req.headers.get("content-type");
  if (ct && hasBody) headers["Content-Type"] = ct;
  const longCode = req.headers.get("x-long-code");
  if (longCode) headers["X-Long-Code"] = longCode;

  let lastError = "Сервер недоступен";
  for (const base of bases) {
    try {
      const res = await fetch(`${base}/${target}${search}`, {
        method: req.method,
        headers,
        body,
        cache: "no-store",
        signal: AbortSignal.timeout(20000),
      });
      const text = await res.text();
      const t = text.trim();
      const looksJson = t.startsWith("{") || t.startsWith("[") || t === "";
      if (res.status >= 500 || (!looksJson && res.status >= 400)) {
        lastError = `HTTP ${res.status}`;
        continue;
      }
      return new Response(t === "" ? '{"code":0}' : text, {
        status: res.status,
        headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store", "x-ax-base": base },
      });
    } catch (e) {
      lastError = e instanceof Error ? e.message : String(e);
    }
  }
  return json({ code: -1, error: `Сеть недоступна: ${lastError}` }, 502);
}

export const GET = handler;
export const POST = handler;
