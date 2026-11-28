"use client";

import { useEffect, useState } from "react";
import { ErrorBox, PosterCard } from "@/components/cards";
import { ax, title, type Release } from "@/lib/client";

const DAYS: [string, string][] = [
  ["monday", "Понедельник"],
  ["tuesday", "Вторник"],
  ["wednesday", "Среда"],
  ["thursday", "Четверг"],
  ["friday", "Пятница"],
  ["saturday", "Суббота"],
  ["sunday", "Воскресенье"],
];

export default function SchedulePage() {
  const [data, setData] = useState<Record<string, Release[]> | null>(null);
  const [err, setErr] = useState("");
  const today = DAYS[(new Date().getDay() + 6) % 7][0];
  const [day, setDay] = useState(today);

  const load = () => {
    setErr("");
    ax("schedule")
      .then((d) => setData(d))
      .catch((e) => setErr(e.message));
  };
  useEffect(load, []);

  return (
    <div>
      <h1 className="mb-4 text-xl font-bold">Расписание выхода серий</h1>
      <div className="mb-5 flex flex-wrap gap-2">
        {DAYS.map(([k, n]) => (
          <button key={k} className={`chip chip-btn ${day === k ? "chip-on" : ""}`} onClick={() => setDay(k)}>
            {n}
            {k === today ? " · сегодня" : ""}
          </button>
        ))}
      </div>
      {err && <ErrorBox text={err} onRetry={load} />}
      {!data && !err && (
        <div className="grid grid-cols-3 gap-3 sm:grid-cols-4 md:grid-cols-6 xl:grid-cols-8">
          {Array.from({ length: 16 }).map((_, i) => (
            <div key={i} className="skeleton rounded-xl" style={{ aspectRatio: "2/3" }} />
          ))}
        </div>
      )}
      {data && (
        <div className="grid grid-cols-3 gap-3 sm:grid-cols-4 md:grid-cols-6 xl:grid-cols-8">
          {((data[day] as Release[]) || []).map((r) => (
            <PosterCard
              key={r.id}
              r={{ id: r.id, image: r.image, title: title(r), sub: r.episodes_released ? `${r.episodes_released} эп.` : r.year }}
              badge={r.last_update_date ? undefined : undefined}
            />
          ))}
          {!((data[day] as Release[]) || []).length && <div className="col-span-full py-12 text-center text-white/45">В этот день релизов нет</div>}
        </div>
      )}
    </div>
  );
}
