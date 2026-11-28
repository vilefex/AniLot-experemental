"use client";

import { Suspense, useCallback, useEffect, useRef, useState } from "react";
import { useParams, useRouter, useSearchParams } from "next/navigation";
import { Loader2 } from "lucide-react";
import Player, { type PlayerEpisode } from "@/components/Player";
import { ax, episodeNumber, getEpisodeTime, saveEpisodeTime, saveProgress, title, type Release } from "@/lib/client";

/* eslint-disable @typescript-eslint/no-explicit-any */
type Ep = { position: number; name: string; url: string };

function Inner() {
  const { id: idParam } = useParams<{ id: string }>();
  const id = Number(idParam);
  const sp = useSearchParams();
  const router = useRouter();
  const type = Number(sp.get("t") || 0);
  const source = Number(sp.get("s") || 0);
  const pos = Number(sp.get("p") || 0);

  const [release, setRelease] = useState<Release | null>(null);
  const [eps, setEps] = useState<Ep[]>([]);
  const [voice, setVoice] = useState("");
  const [srcName, setSrcName] = useState("");
  const [url, setUrl] = useState("");
  const [err, setErr] = useState("");
  const [startAt, setStartAt] = useState(0);
  const lastSync = useRef(0);

  // метаданные: релиз, озвучка, источник, список серий
  useEffect(() => {
    let live = true;
    (async () => {
      try {
        const [rel, types, srcs, list] = await Promise.all([
          ax(`release/${id}`),
          ax(`episode/${id}`),
          ax(`episode/${id}/${type}`),
          ax(`episode/${id}/${type}/${source}`),
        ]);
        if (!live) return;
        if (rel.release) setRelease(rel.release);
        setVoice((types.types || []).find((v: any) => v.id === type)?.name || "");
        setSrcName((srcs.sources || []).find((s: any) => s.id === source)?.name || "");
        setEps(((list.episodes || []) as Ep[]).map((e) => ({ position: e.position, name: e.name, url: e.url })));
      } catch (e) {
        if (live) setErr(e instanceof Error ? e.message : "Ошибка загрузки");
      }
    })();
    return () => {
      live = false;
    };
  }, [id, type, source]);

  // ссылка на конкретную серию (свежая подписанная ссылка через episode/target)
  useEffect(() => {
    let live = true;
    setUrl("");
    setErr("");
    const t = getEpisodeTime(id, source, pos);
    setStartAt(t && !t.done && t.d && t.t / t.d < 0.95 ? t.t : 0);
    (async () => {
      try {
        const d = await ax(`episode/target/${id}/${source}/${pos}`);
        let u: string = d.episode?.url || "";
        if (!u) {
          const list = await ax(`episode/${id}/${type}/${source}`);
          u = (list.episodes || []).find((e: Ep) => e.position === pos)?.url || "";
        }
        if (!live) return;
        if (!u) throw new Error(d.code === 2 ? "Серия недоступна" : "Не удалось получить ссылку на серию");
        setUrl(u);
      } catch (e) {
        if (live) setErr(e instanceof Error ? e.message : "Ошибка загрузки");
      }
    })();
    return () => {
      live = false;
    };
  }, [id, source, type, pos]);

  const current = eps.find((e) => e.position === pos);
  const episodeName = current?.name || `Серия ${pos}`;
  const idx = eps.findIndex((e) => e.position === pos);
  const number = episodeNumber(episodeName, idx >= 0 ? idx + 1 : pos);

  const onProgress = useCallback(
    (time: number, duration: number) => {
      if (!release) return;
      saveProgress({
        release: id, title: title(release), image: release.image || "", type, source, position: pos,
        epName: episodeName, time, duration, at: Date.now(),
      });
      saveEpisodeTime(id, source, pos, time, duration);
    },
    [release, id, type, source, pos, episodeName],
  );

  const onWatched = useCallback(() => {
    saveEpisodeTime(id, source, pos, 0, 0, true);
    if (Date.now() - lastSync.current < 3000) return;
    lastSync.current = Date.now();
    ax(`episode/watch/${id}/${source}/${pos}`, { method: "POST" }).catch(() => {});
    ax(`history/add/${id}/${source}/${pos}`).catch(() => {});
  }, [id, source, pos]);

  const back = () => router.push(`/release/${id}`);
  const select = (p: number) => router.replace(`/watch/${id}?t=${type}&s=${source}&p=${p}`);

  if (err && !url) {
    return (
      <div className="grid h-screen place-items-center bg-black">
        <div className="max-w-md rounded-2xl border border-white/10 bg-surface p-6 text-center">
          <p className="mb-4 text-sm text-white/80">{err}</p>
          <button className="btn btn-primary" onClick={back}>
            К релизу
          </button>
        </div>
      </div>
    );
  }
  if (!url || !release) {
    return (
      <div className="grid h-screen place-items-center bg-black">
        <Loader2 className="animate-spin text-accent" size={48} />
      </div>
    );
  }

  const playerEps: PlayerEpisode[] = eps.map((e) => ({ position: e.position, name: e.name }));
  return (
    <Player
      key={`${id}:${source}:${pos}`}
      releaseId={id}
      title={title(release)}
      titleOriginal={release.title_original || ""}
      year={Number(release.year) || 0}
      episodeName={episodeName}
      episodeNumber={number}
      voiceName={voice}
      sourceName={srcName}
      url={url}
      episodes={playerEps}
      currentPosition={pos}
      startAt={startAt}
      onBack={back}
      onSelect={select}
      onProgress={onProgress}
      onWatched={onWatched}
    />
  );
}

export default function WatchPage() {
  return (
    <Suspense>
      <Inner />
    </Suspense>
  );
}
