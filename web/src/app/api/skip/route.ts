import type { NextRequest } from "next/server";
import { json } from "@/lib/server/net";

// AniSkip: сопоставление релиза с MAL ID через Shikimori + метки OP/ED.
export const dynamic = "force-dynamic";

const SHIKI_HOSTS = ["https://shikimori.one", "https://shikimori.io", "https://shikimori.me"];
const malCache = new Map<string, number | null>();
const markCache = new Map<string, { at: number; data: unknown }>();

const norm = (s: string) => s.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "");

async function shiki(path: string): Promise<unknown> {
  for (const host of SHIKI_HOSTS) {
    try {
      const r = await fetch(host + path, {
        headers: { "User-Agent": "AniLot", Accept: "application/json" },
        redirect: "follow",
        signal: AbortSignal.timeout(8000),
      });
      if (r.ok) return await r.json();
    } catch {
      // следующее зеркало
    }
  }
  return null;
}

async function findMal(titles: string[], year: number): Promise<number | null> {
  const key = `${titles[0]}|${year}`;
  if (malCache.has(key)) return malCache.get(key) ?? null;
  let found: number | null = null;
  for (const t of titles.filter(Boolean).slice(0, 3)) {
    const list = (await shiki(`/api/animes?search=${encodeURIComponent(t)}&limit=10&order=popularity`)) as
      | { id: number; name: string; russian: string; aired_on: string | null }[]
      | null;
    if (!Array.isArray(list)) continue;
    const want = titles.map(norm);
    const exact = list.filter((a) => want.includes(norm(a.name || "")) || want.includes(norm(a.russian || "")));
    const pool = exact.length ? exact : [];
    const byYear = year ? pool.filter((a) => a.aired_on?.startsWith(String(year))) : pool;
    const pick = byYear.length === 1 ? byYear[0] : pool.length === 1 ? pool[0] : null;
    if (pick) {
      found = pick.id;
      break;
    }
  }
  malCache.set(key, found);
  return found;
}

export async function GET(req: NextRequest): Promise<Response> {
  const p = req.nextUrl.searchParams;
  const titles = [p.get("title") || "", p.get("alt") || "", p.get("ru") || ""];
  const year = Number(p.get("year") || 0);
  const ep = Number(p.get("ep") || 0);
  if (!titles[0] || !ep) return json({ markers: [] });

  const mal = await findMal(titles, year);
  if (!mal) return json({ markers: [], mal: null });

  const key = `${mal}:${ep}`;
  const cached = markCache.get(key);
  if (cached && Date.now() - cached.at < 6 * 3600 * 1000) return json(cached.data);

  try {
    const r = await fetch(
      `https://api.aniskip.com/v2/skip-times/${mal}/${ep}?types[]=op&types[]=ed&types[]=recap&episodeLength=0`,
      { signal: AbortSignal.timeout(8000) },
    );
    const d = (await r.json()) as {
      found?: boolean;
      results?: { interval: { startTime: number; endTime: number }; skipType: string }[];
    };
    const markers = (d.results || []).map((x) => ({ type: x.skipType, start: x.interval.startTime, end: x.interval.endTime }));
    const data = { markers, mal };
    markCache.set(key, { at: Date.now(), data });
    return json(data);
  } catch {
    return json({ markers: [], mal });
  }
}
