"use client";

import Link from "next/link";
import { Star } from "lucide-react";
import { Poster } from "@/components/Poster";
import { title, type Release } from "@/lib/client";

/** Горизонтальная карточка каталога (как в AniLibrix): постер слева, описание справа. */
export function ReleaseRow({ r }: { r: Release }) {
  const meta = [r.year, r.category?.name, r.status?.name].filter(Boolean);
  const eps = r.episodes_total
    ? `${r.episodes_released || 0} / ${r.episodes_total} эп.`
    : r.episodes_released
      ? `${r.episodes_released} эп.`
      : "";
  return (
    <Link
      href={`/release/${r.id}`}
      className="card-hover flex gap-4 rounded-xl border border-line bg-surface p-3 hover:border-white/15"
    >
      <Poster src={r.image} alt={title(r)} ratio="2/3" className="w-[120px] shrink-0 rounded-lg sm:w-[140px]" />
      <div className="flex min-w-0 flex-1 flex-col py-1">
        <h3 className="clamp-2 text-lg font-bold leading-snug">{title(r)}</h3>
        {r.title_original && <div className="mt-0.5 truncate text-sm text-white/50">{r.title_original}</div>}
        {r.genres && <div className="mt-1 truncate text-xs text-white/40">{String(r.genres).replace(/, /g, " | ")}</div>}
        <div className="mt-3 flex flex-wrap items-center gap-2">
          {r.grade ? (
            <span className="chip !text-accent">
              <Star size={13} fill="currentColor" /> {Number(r.grade).toFixed(1)}
            </span>
          ) : null}
          {meta.map((m) => (
            <span className="chip" key={String(m)}>
              {String(m)}
            </span>
          ))}
          {eps && <span className="chip">{eps}</span>}
        </div>
        {r.description && <p className="clamp-4 mt-3 hidden text-sm text-white/55 sm:block">{String(r.description).replace(/<[^>]+>/g, "")}</p>}
      </div>
    </Link>
  );
}

/** Вертикальная карточка-постер. */
export function PosterCard({
  r,
  href,
  badge,
  progress,
}: {
  r: { id: number; image?: string; title: string; sub?: string };
  href?: string;
  badge?: string;
  progress?: number;
}) {
  return (
    <Link href={href || `/release/${r.id}`} className="group block w-full">
      <div className="relative overflow-hidden rounded-xl ring-1 ring-white/5 transition group-hover:ring-accent/60">
        <Poster src={r.image} alt={r.title} className="w-full transition duration-300 group-hover:scale-105" />
        {badge && (
          <span className="absolute left-2 top-2 rounded-md bg-black/70 px-2 py-0.5 text-xs font-semibold text-accent backdrop-blur">{badge}</span>
        )}
        {progress !== undefined && progress > 0 && (
          <div className="absolute inset-x-0 bottom-0 h-1 bg-black/60">
            <div className="h-full bg-accent" style={{ width: `${Math.min(100, progress)}%` }} />
          </div>
        )}
      </div>
      <div className="mt-2 clamp-2 text-sm font-semibold leading-snug group-hover:text-accent">{r.title}</div>
      {r.sub && <div className="mt-0.5 truncate text-xs text-white/45">{r.sub}</div>}
    </Link>
  );
}

export function RowSkeleton({ n = 6 }: { n?: number }) {
  return (
    <>
      {Array.from({ length: n }).map((_, i) => (
        <div key={i} className="flex gap-4 rounded-xl border border-line bg-surface p-3">
          <div className="skeleton w-[130px] shrink-0 rounded-lg" style={{ aspectRatio: "2/3" }} />
          <div className="flex-1 space-y-3 py-2">
            <div className="skeleton h-5 w-3/4 rounded" />
            <div className="skeleton h-4 w-1/2 rounded" />
            <div className="skeleton h-4 w-1/3 rounded" />
          </div>
        </div>
      ))}
    </>
  );
}

export function ErrorBox({ text, onRetry }: { text: string; onRetry?: () => void }) {
  return (
    <div className="rounded-xl border border-bad/30 bg-bad/10 p-5 text-sm text-red-200">
      {text}
      {onRetry && (
        <button className="btn ml-4 !h-8" onClick={onRetry}>
          Повторить
        </button>
      )}
    </div>
  );
}
