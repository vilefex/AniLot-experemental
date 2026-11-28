"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { Loader2 } from "lucide-react";
import { ErrorBox, ReleaseRow, RowSkeleton } from "@/components/cards";
import type { Release } from "@/lib/client";

type Page = { code?: number; content?: Release[]; total_page_count?: number; current_page?: number };

/** Постраничный список релизов с кнопкой «Показать ещё» и автоподгрузкой при прокрутке. */
export default function PagedList({
  load,
  resetKey,
  empty = "Ничего не найдено",
}: {
  load: (page: number) => Promise<Page>;
  resetKey: string;
  empty?: string;
}) {
  const [items, setItems] = useState<Release[]>([]);
  const [page, setPage] = useState(0);
  const [pages, setPages] = useState(1);
  const [loading, setLoading] = useState(true);
  const [err, setErr] = useState("");
  const loadRef = useRef(load);
  loadRef.current = load;
  const token = useRef(0);
  const sentinel = useRef<HTMLDivElement>(null);

  const fetchPage = useCallback(async (n: number, reset: boolean) => {
    const my = ++token.current;
    setLoading(true);
    setErr("");
    try {
      const d = await loadRef.current(n);
      if (my !== token.current) return;
      if (d.code && d.code !== 0) throw new Error(`Сервер вернул код ${d.code}`);
      const list = d.content || [];
      setItems((prev) => {
        const base = reset ? [] : prev;
        const seen = new Set(base.map((x) => x.id));
        return [...base, ...list.filter((x) => !seen.has(x.id))];
      });
      setPage(n);
      setPages(d.total_page_count ?? (list.length ? n + 2 : n + 1));
    } catch (e) {
      if (my === token.current) setErr(e instanceof Error ? e.message : "Ошибка загрузки");
    } finally {
      if (my === token.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    setItems([]);
    setPages(1);
    fetchPage(0, true);
  }, [resetKey, fetchPage]);

  const hasMore = page + 1 < pages;

  useEffect(() => {
    const el = sentinel.current;
    if (!el || !hasMore || loading || err) return;
    const io = new IntersectionObserver(
      (e) => {
        if (e[0].isIntersecting) fetchPage(page + 1, false);
      },
      { rootMargin: "400px" },
    );
    io.observe(el);
    return () => io.disconnect();
  }, [hasMore, loading, err, page, fetchPage]);

  return (
    <div>
      <div className="grid gap-3 xl:grid-cols-2">
        {items.map((r) => (
          <ReleaseRow key={r.id} r={r} />
        ))}
        {loading && items.length === 0 && <RowSkeleton />}
      </div>
      {err && <ErrorBox text={err} onRetry={() => fetchPage(items.length ? page + 1 : 0, items.length === 0)} />}
      {!loading && !err && items.length === 0 && <div className="py-16 text-center text-white/45">{empty}</div>}
      <div ref={sentinel} className="flex h-16 items-center justify-center">
        {loading && items.length > 0 && <Loader2 className="animate-spin text-accent" />}
        {!loading && hasMore && !err && (
          <button className="btn" onClick={() => fetchPage(page + 1, false)}>
            Показать ещё
          </button>
        )}
      </div>
    </div>
  );
}
