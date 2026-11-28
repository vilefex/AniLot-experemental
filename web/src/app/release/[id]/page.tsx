"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import { ArrowDownUp, Check, Heart, Play, Search, Star } from "lucide-react";
import { ErrorBox, PosterCard } from "@/components/cards";
import { Poster } from "@/components/Poster";
import {
  ax, LIST_STATUSES, title, useAuth, useEpisodeTimes, useProgress, type Release,
} from "@/lib/client";

/* eslint-disable @typescript-eslint/no-explicit-any */
type Voice = { id: number; name: string; workers?: string; episodes_count: number; is_sub: boolean; pinned: boolean };
type Src = { id: number; name: string; episodes_count: number };
type Ep = { position: number; name: string; url: string; is_watched?: boolean; is_filler?: boolean };

export default function ReleasePage() {
  const { id: idParam } = useParams<{ id: string }>();
  const id = Number(idParam);
  const auth = useAuth();
  const progress = useProgress()[String(id)];
  const times = useEpisodeTimes();

  const [r, setR] = useState<Release | null>(null);
  const [err, setErr] = useState("");
  const [tab, setTab] = useState<"eps" | "info" | "more">("eps");
  const [fav, setFav] = useState(false);
  const [listSt, setListSt] = useState(0);

  const [voices, setVoices] = useState<Voice[] | null>(null);
  const [voice, setVoice] = useState(0);
  const [sources, setSources] = useState<Src[]>([]);
  const [source, setSource] = useState(0);
  const [eps, setEps] = useState<Ep[] | null>(null);
  const [epErr, setEpErr] = useState("");
  const [q, setQ] = useState("");
  const [desc, setDesc] = useState(false);

  const load = useCallback(() => {
    setErr("");
    ax(`release/${id}`)
      .then((d) => {
        if (d.code !== 0 || !d.release) throw new Error(d.code === 2 ? "Релиз не найден" : `Ошибка сервера (код ${d.code})`);
        setR(d.release);
        setFav(!!d.release.is_favorite);
        setListSt(d.release.profile_list_status || 0);
      })
      .catch((e) => setErr(e.message));
  }, [id]);
  useEffect(load, [load]);

  // озвучки
  useEffect(() => {
    setVoices(null);
    ax(`episode/${id}`)
      .then((d) => {
        const list: Voice[] = d.types || [];
        setVoices(list);
        const pref = progress && list.find((v) => v.id === progress.type);
        const first = pref || list.find((v) => v.pinned) || [...list].sort((a, b) => b.episodes_count - a.episodes_count)[0];
        setVoice(first?.id || 0);
      })
      .catch((e) => setEpErr(e.message));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  // источники
  useEffect(() => {
    if (!voice) return;
    setSources([]);
    setSource(0);
    setEps(null);
    ax(`episode/${id}/${voice}`)
      .then((d) => {
        const list: Src[] = d.sources || [];
        setSources(list);
        const pref = progress && list.find((s) => s.id === progress.source);
        setSource((pref || list[0])?.id || 0);
      })
      .catch((e) => setEpErr(e.message));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, voice]);

  // серии
  useEffect(() => {
    if (!voice || !source) return;
    setEps(null);
    setEpErr("");
    ax(`episode/${id}/${voice}/${source}`)
      .then((d) => setEps(d.episodes || []))
      .catch((e) => setEpErr(e.message));
  }, [id, voice, source]);

  const shown = useMemo(() => {
    let list = eps || [];
    if (q.trim()) list = list.filter((e) => e.name.toLowerCase().includes(q.trim().toLowerCase()));
    return desc ? [...list].reverse() : list;
  }, [eps, q, desc]);

  const watchHref = (pos: number) => `/watch/${id}?t=${voice}&s=${source}&p=${pos}`;
  const resume = progress && eps?.find((e) => e.position === progress.position && progress.source === source);
  const firstEp = eps?.[0];

  const toggleFav = async () => {
    if (!auth.token) return alert("Войдите в аккаунт, чтобы пользоваться избранным");
    const on = !fav;
    setFav(on);
    try {
      await ax(`favorite/${on ? "add" : "delete"}/${id}`);
    } catch {
      setFav(!on);
    }
  };
  const changeList = async (n: number) => {
    if (!auth.token) return alert("Войдите в аккаунт, чтобы пользоваться списками");
    const old = listSt;
    setListSt(n);
    try {
      if (old > 0 && old !== n) await ax(`profile/list/delete/${old}/${id}`);
      if (n > 0) await ax(`profile/list/add/${n}/${id}`);
    } catch {
      setListSt(old);
    }
  };

  if (err) return <ErrorBox text={err} onRetry={load} />;
  if (!r) {
    return (
      <div className="flex gap-6">
        <div className="skeleton w-56 shrink-0 rounded-2xl" style={{ aspectRatio: "2/3" }} />
        <div className="flex-1 space-y-4 pt-4">
          <div className="skeleton h-9 w-2/3 rounded" />
          <div className="skeleton h-5 w-1/3 rounded" />
          <div className="skeleton h-24 w-full rounded" />
        </div>
      </div>
    );
  }

  const related: Release[] = [...(r.related_releases || []), ...(r.recommended_releases || [])];
  const info: [string, any][] = [
    ["Тип", r.category?.name], ["Статус", r.status?.name], ["Год", r.year], ["Страна", r.country], ["Студия", r.studio],
    ["Режиссёр", r.director], ["Автор", r.author], ["Источник", r.source], ["Длительность", r.duration ? `${r.duration} мин.` : ""],
    ["Эпизоды", r.episodes_total ? `${r.episodes_released || 0} из ${r.episodes_total}` : r.episodes_released],
    ["Озвучка", r.translators], ["Возраст", r.age_rating ? `${r.age_rating}+` : ""],
  ];

  return (
    <div className="relative">
      {/* размытый фон */}
      <div className="pointer-events-none absolute -inset-x-[4%] -top-20 h-[420px] overflow-hidden opacity-30">
        <Poster src={r.image} ratio="auto" className="h-full w-full scale-110 blur-3xl" />
        <div className="absolute inset-0 bg-gradient-to-b from-transparent via-bg/70 to-bg" />
      </div>

      <div className="relative flex flex-col gap-6 pt-4 md:flex-row">
        <div className="mx-auto w-48 shrink-0 md:mx-0 md:w-60">
          <Poster src={r.image} alt={title(r)} priority className="w-full rounded-2xl shadow-2xl ring-1 ring-white/10" />
        </div>
        <div className="min-w-0 flex-1">
          <h1 className="text-3xl font-black leading-tight md:text-4xl">{title(r)}</h1>
          {r.title_original && <div className="mt-1 text-white/55">{r.title_original}</div>}
          {r.genres && <div className="mt-1 text-sm text-white/40">{String(r.genres).replace(/, /g, " | ")}</div>}

          <div className="mt-4 flex flex-wrap gap-2">
            {r.grade ? (
              <span className="chip !text-accent">
                <Star size={14} fill="currentColor" /> {Number(r.grade).toFixed(2)} · {r.vote_count || 0}
              </span>
            ) : null}
            {[r.year, r.category?.name, r.status?.name, r.season ? ["", "Зима", "Весна", "Лето", "Осень"][r.season] : ""]
              .filter(Boolean)
              .map((x) => (
                <span className="chip" key={String(x)}>
                  {String(x)}
                </span>
              ))}
          </div>

          <div className="mt-5 flex flex-wrap items-center gap-3">
            {eps && eps.length > 0 ? (
              <Link href={watchHref(resume ? resume.position : firstEp!.position)} className="btn btn-primary !h-11 !px-6">
                <Play size={18} fill="currentColor" />
                {resume ? `Продолжить · ${resume.name}` : "Смотреть"}
              </Link>
            ) : (
              <button className="btn btn-primary !h-11 !px-6" disabled>
                <Play size={18} fill="currentColor" /> {eps === null && !epErr && voices !== null && voices.length ? "Загрузка…" : "Нет серий"}
              </button>
            )}
            <button className={`btn !h-11 ${fav ? "!text-bad" : ""}`} onClick={toggleFav}>
              <Heart size={18} fill={fav ? "currentColor" : "none"} /> {fav ? "В избранном" : "В избранное"}
            </button>
            <select className="btn !h-11 appearance-none !pr-4" value={listSt} onChange={(e) => changeList(Number(e.target.value))}>
              <option value={0}>Не в списке</option>
              {LIST_STATUSES.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>
          </div>

          {r.description && <p className="mt-5 max-w-3xl whitespace-pre-line text-[15px] leading-relaxed text-white/75">{String(r.description).replace(/<[^>]+>/g, "")}</p>}
        </div>
      </div>

      {/* вкладки */}
      <div className="relative mt-8 flex gap-1 border-b border-line">
        {([["eps", "Эпизоды"], ["info", "Информация"], ["more", `Похожее${related.length ? ` (${related.length})` : ""}`]] as const).map(([k, n]) => (
          <button
            key={k}
            onClick={() => setTab(k)}
            className={`-mb-px border-b-2 px-5 py-3 text-sm font-semibold transition ${tab === k ? "border-accent text-accent" : "border-transparent text-white/55 hover:text-white"}`}
          >
            {n}
          </button>
        ))}
      </div>

      <div className="relative mt-5">
        {tab === "eps" && (
          <div>
            {voices && voices.length === 0 && <div className="py-10 text-center text-white/45">Для этого релиза нет доступных серий</div>}
            {voices && voices.length > 0 && (
              <div className="space-y-3">
                <div className="flex flex-wrap gap-2">
                  {voices.map((v) => (
                    <button key={v.id} className={`chip chip-btn ${voice === v.id ? "chip-on" : ""}`} onClick={() => setVoice(v.id)}>
                      {v.name}
                      {v.is_sub ? " · саб" : ""} <span className="opacity-60">{v.episodes_count}</span>
                    </button>
                  ))}
                </div>
                {sources.length > 0 && (
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-xs uppercase tracking-wider text-white/35">Плеер:</span>
                    {sources.map((s) => (
                      <button key={s.id} className={`chip chip-btn ${source === s.id ? "chip-on" : ""}`} onClick={() => setSource(s.id)}>
                        {s.name}
                      </button>
                    ))}
                  </div>
                )}
                <div className="flex items-center gap-2">
                  <div className="relative w-64">
                    <Search size={15} className="absolute left-3 top-1/2 -translate-y-1/2 text-white/40" />
                    <input className="input !h-9 !pl-9" placeholder="Поиск серии" value={q} onChange={(e) => setQ(e.target.value)} />
                  </div>
                  <button className="btn !h-9 !px-3" onClick={() => setDesc((d) => !d)} title="Сортировка">
                    <ArrowDownUp size={16} />
                  </button>
                </div>
              </div>
            )}
            {epErr && <div className="mt-3"><ErrorBox text={epErr} /></div>}
            <div className="mt-4 grid gap-2 md:grid-cols-2 xl:grid-cols-3">
              {eps === null && voice > 0 && !epErr && Array.from({ length: 6 }).map((_, i) => <div key={i} className="skeleton h-14 rounded-xl" />)}
              {shown.map((e) => {
                const t = times[`${id}:${source}:${e.position}`];
                const watched = e.is_watched || t?.done;
                const pct = t && t.d ? (t.t / t.d) * 100 : 0;
                const current = progress && progress.source === source && progress.position === e.position;
                return (
                  <Link
                    key={e.position}
                    href={watchHref(e.position)}
                    className={`card-hover relative flex items-center gap-3 overflow-hidden rounded-xl border bg-surface px-4 py-3 ${current ? "border-accent/60" : "border-line"}`}
                  >
                    <span className={`grid h-8 w-8 shrink-0 place-items-center rounded-full ${watched ? "bg-ok/20 text-ok" : "bg-accent-soft text-accent"}`}>
                      {watched ? <Check size={16} /> : <Play size={14} fill="currentColor" />}
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className={`block truncate text-sm font-semibold ${watched ? "text-white/55" : ""}`}>{e.name}</span>
                      {e.is_filler && <span className="text-xs text-white/35">Филлер</span>}
                    </span>
                    {pct > 0 && !watched && <span className="absolute inset-x-0 bottom-0 h-0.5 bg-accent" style={{ width: `${Math.min(100, pct)}%` }} />}
                  </Link>
                );
              })}
            </div>
          </div>
        )}

        {tab === "info" && (
          <dl className="grid max-w-3xl gap-x-8 gap-y-3 sm:grid-cols-2">
            {info.filter(([, v]) => v).map(([k, v]) => (
              <div key={k} className="flex justify-between gap-4 border-b border-line py-2 text-sm">
                <dt className="text-white/45">{k}</dt>
                <dd className="text-right font-medium">{String(v)}</dd>
              </div>
            ))}
          </dl>
        )}

        {tab === "more" && (
          <div className="grid grid-cols-3 gap-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6 xl:grid-cols-8">
            {related.map((x) => (
              <PosterCard key={x.id} r={{ id: x.id, image: x.image, title: title(x), sub: x.year }} />
            ))}
            {related.length === 0 && <div className="col-span-full py-10 text-center text-white/45">Нет связанных релизов</div>}
          </div>
        )}
      </div>
    </div>
  );
}

