"use client";

import { useMemo, useState } from "react";
import { PosterCard } from "@/components/cards";
import PagedList from "@/components/PagedList";
import { ax, EMPTY_FILTER, GENRES, SORTS, useProgress } from "@/lib/client";

export default function HomePage() {
  const [sort, setSort] = useState(0);
  const [genres, setGenres] = useState<string[]>([]);
  const [year, setYear] = useState<number | null>(null);
  const progress = useProgress();

  const cont = useMemo(
    () => Object.values(progress).sort((a, b) => b.at - a.at).slice(0, 12),
    [progress],
  );
  const years = useMemo(() => {
    const y = new Date().getFullYear();
    return Array.from({ length: 30 }, (_, i) => y - i);
  }, []);

  const filter = { ...EMPTY_FILTER, sort, genres, start_year: year, end_year: year };
  const key = JSON.stringify(filter);

  return (
    <div className="space-y-8">
      {cont.length > 0 && (
        <section>
          <h2 className="mb-3 text-xl font-bold">Продолжить просмотр</h2>
          <div className="grid grid-cols-3 gap-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6 xl:grid-cols-8">
            {cont.map((p) => (
              <PosterCard
                key={p.release}
                r={{ id: p.release, image: p.image, title: p.title, sub: p.epName }}
                href={`/watch/${p.release}?t=${p.type}&s=${p.source}&p=${p.position}`}
                progress={p.duration ? (p.time / p.duration) * 100 : 0}
              />
            ))}
          </div>
        </section>
      )}

      <section>
        <div className="mb-4 flex flex-wrap items-center gap-2">
          <h2 className="mr-3 text-xl font-bold">Каталог</h2>
          {SORTS.map((s) => (
            <button key={s.id} className={`chip chip-btn ${sort === s.id ? "chip-on" : ""}`} onClick={() => setSort(s.id)}>
              {s.name}
            </button>
          ))}
          <select
            className="chip chip-btn ml-auto !pr-2"
            value={year ?? ""}
            onChange={(e) => setYear(e.target.value ? Number(e.target.value) : null)}
          >
            <option value="">Все годы</option>
            {years.map((y) => (
              <option key={y} value={y}>
                {y}
              </option>
            ))}
          </select>
        </div>
        <div className="mb-5 flex flex-wrap gap-2">
          {GENRES.map((g) => (
            <button
              key={g}
              className={`chip chip-btn ${genres.includes(g) ? "chip-on" : ""}`}
              onClick={() => setGenres((p) => (p.includes(g) ? p.filter((x) => x !== g) : [...p, g]))}
            >
              {g}
            </button>
          ))}
        </div>
        <PagedList resetKey={key} load={(page) => ax(`filter/${page}`, { json: filter })} />
      </section>
    </div>
  );
}
