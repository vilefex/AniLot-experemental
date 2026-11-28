"use client";

import { useState } from "react";
import { CheckCircle2, Download, Loader2, XCircle } from "lucide-react";
import { getSettings, useSettings } from "@/lib/client";

const SAMPLE = "http://s.anixmirai.com/posters/DhKVWCzSvQEltbsRr60trn2eXTCdNu.jpg"; // http:// — проверяем https-апгрейд

type Res = { ok: boolean; ms: number; note?: string };

function loadImg(src: string, timeout = 8000): Promise<Res> {
  return new Promise((resolve) => {
    const t0 = performance.now();
    const img = new Image();
    img.referrerPolicy = "no-referrer";
    const done = (ok: boolean) => {
      clearTimeout(tm);
      resolve({ ok, ms: Math.round(performance.now() - t0) });
    };
    const tm = setTimeout(() => done(false), timeout);
    img.onload = () => done(true);
    img.onerror = () => done(false);
    img.src = src;
  });
}

function Row({ title, hint, children }: { title: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3 border-b border-line py-4 last:border-0">
      <div className="max-w-md">
        <div className="font-medium">{title}</div>
        {hint && <div className="mt-0.5 text-xs text-white/45">{hint}</div>}
      </div>
      <div className="flex flex-wrap items-center gap-2">{children}</div>
    </div>
  );
}

function Toggle({ on, set }: { on: boolean; set: (v: boolean) => void }) {
  return (
    <button onClick={() => set(!on)} className={`relative h-6 w-11 rounded-full transition ${on ? "bg-accent" : "bg-white/20"}`}>
      <span className={`absolute top-0.5 h-5 w-5 rounded-full bg-white transition-all ${on ? "left-[22px]" : "left-0.5"}`} />
    </button>
  );
}

export default function SettingsPage() {
  const [s, set] = useSettings();
  const [apiTest, setApiTest] = useState<null | "run" | { ok: boolean; base?: string; ms: number; note?: string }>(null);
  const [imgTest, setImgTest] = useState<null | "run" | Record<string, Res>>(null);

  const testApi = async () => {
    setApiTest("run");
    const t0 = performance.now();
    try {
      const cur = getSettings();
      const headers: Record<string, string> = { "x-ax-mode": cur.conn };
      if (cur.custom.trim()) headers["x-ax-custom"] = cur.custom.trim();
      const r = await fetch("/api/ax/schedule", { headers });
      const d = await r.json();
      setApiTest({ ok: r.ok && d.code === 0, base: r.headers.get("x-ax-base") || undefined, ms: Math.round(performance.now() - t0), note: d.error });
    } catch (e) {
      setApiTest({ ok: false, ms: Math.round(performance.now() - t0), note: e instanceof Error ? e.message : "ошибка" });
    }
  };

  const testImg = async () => {
    setImgTest("run");
    const https = SAMPLE.replace(/^http:/, "https:");
    const [direct, proxy, alt] = await Promise.all([
      loadImg(https),
      loadImg(`/api/img?u=${encodeURIComponent(https)}&m=proxy&_=${Date.now()}`),
      loadImg(`/api/img?u=${encodeURIComponent(https)}&m=alt&_=${Date.now()}`),
    ]);
    setImgTest({ "Напрямую (https)": direct, "Через прокси": proxy, "Альтернативное подключение": alt });
  };

  const seg = <T extends string>(val: T, cur: T, on: () => void, label: string) => (
    <button key={val} className={`chip chip-btn ${val === cur ? "chip-on" : ""}`} onClick={on}>
      {label}
    </button>
  );

  return (
    <div className="mx-auto max-w-3xl">
      <h1 className="mb-6 text-2xl font-bold">Настройки</h1>

      <section className="mb-6 rounded-2xl border border-line bg-surface px-6 py-2">
        <h2 className="pt-4 text-sm font-semibold uppercase tracking-wider text-accent">Подключение к API</h2>
        <Row title="Адрес сервера" hint="Авто: основной адрес, при сбое — запасной. Альтернативное: сначала запасной адрес.">
          {seg("auto", s.conn, () => set({ conn: "auto" }), "Авто")}
          {seg("alt", s.conn, () => set({ conn: "alt" }), "Альтернативное подключение")}
        </Row>
        <Row title="Собственный адрес API" hint="Например, свой прокси. Если указан — используется только он.">
          <input
            className="input !w-72"
            placeholder="https://api.example.com"
            value={s.custom}
            onChange={(e) => set({ custom: e.target.value })}
          />
        </Row>
        <Row title="Проверка соединения">
          <button className="btn" onClick={testApi} disabled={apiTest === "run"}>
            {apiTest === "run" && <Loader2 size={16} className="animate-spin" />} Проверить
          </button>
          {apiTest && apiTest !== "run" && (
            <span className={`flex items-center gap-1.5 text-sm ${apiTest.ok ? "text-ok" : "text-bad"}`}>
              {apiTest.ok ? <CheckCircle2 size={16} /> : <XCircle size={16} />}
              {apiTest.ok ? `${apiTest.base?.replace("https://", "")} · ${apiTest.ms} мс` : apiTest.note || "недоступно"}
            </span>
          )}
        </Row>
      </section>

      <section className="mb-6 rounded-2xl border border-line bg-surface px-6 py-2">
        <h2 className="pt-4 text-sm font-semibold uppercase tracking-wider text-accent">Картинки</h2>
        <Row
          title="Способ загрузки"
          hint="Авто: https-апгрейд → прокси-fallback → альтернативное подключение. Рабочий способ запоминается для каждого хоста."
        >
          {seg("auto", s.img, () => set({ img: "auto" }), "Авто")}
          {seg("direct", s.img, () => set({ img: "direct" }), "Напрямую")}
          {seg("proxy", s.img, () => set({ img: "proxy" }), "Прокси")}
          {seg("alt", s.img, () => set({ img: "alt" }), "Альтернативный")}
        </Row>
        <Row title="Проверка способов">
          <button className="btn" onClick={testImg} disabled={imgTest === "run"}>
            {imgTest === "run" && <Loader2 size={16} className="animate-spin" />} Проверить
          </button>
          {imgTest && imgTest !== "run" && (
            <div className="w-full space-y-1 text-sm">
              {Object.entries(imgTest).map(([k, v]) => (
                <div key={k} className={`flex items-center gap-2 ${v.ok ? "text-ok" : "text-bad"}`}>
                  {v.ok ? <CheckCircle2 size={15} /> : <XCircle size={15} />}
                  <span className="text-white/80">{k}</span>
                  <span className="text-white/40">{v.ok ? `${v.ms} мс` : "не удалось"}</span>
                </div>
              ))}
            </div>
          )}
        </Row>
      </section>

      <section className="mb-6 rounded-2xl border border-line bg-surface px-6 py-2">
        <h2 className="pt-4 text-sm font-semibold uppercase tracking-wider text-accent">Плеер</h2>
        <Row title="Автопропуск опенинга/эндинга" hint="Метки берутся из AniSkip (сопоставление через Shikimori).">
          <Toggle on={s.autoSkip} set={(v) => set({ autoSkip: v })} />
        </Row>
        <Row title="Автопереход на следующую серию">
          <Toggle on={s.autoNext} set={(v) => set({ autoNext: v })} />
        </Row>
        <Row title="Предпочитаемое качество">
          {[0, 1080, 720, 480, 360].map((q) => (
            <button key={q} className={`chip chip-btn ${s.quality === q ? "chip-on" : ""}`} onClick={() => set({ quality: q })}>
              {q ? `${q}p` : "Максимум"}
            </button>
          ))}
        </Row>
        <Row title="Скорость по умолчанию">
          {[0.75, 1, 1.25, 1.5, 2].map((x) => (
            <button key={x} className={`chip chip-btn ${s.speed === x ? "chip-on" : ""}`} onClick={() => set({ speed: x })}>
              ×{x}
            </button>
          ))}
        </Row>
        <div className="py-4 text-xs leading-relaxed text-white/45">
          <b className="text-white/70">Горячие клавиши:</b> Space/K — пауза · ←/→ — ±5 с (Shift — ±30 с) · J/L — ±10 с · ↑/↓ — громкость · M — звук · F —
          полный экран · P — PiP · [ ] — скорость · Backspace — ×1 · S — пропуск OP/ED · N/B — следующая/предыдущая серия · Esc — назад.
        </div>
      </section>

      <section className="rounded-2xl border border-line bg-surface px-6 py-4">
        <h2 className="text-sm font-semibold uppercase tracking-wider text-accent">Патч для репозитория</h2>
        <p className="mt-2 text-sm text-white/55">
          Готовый патч для vilefex/AniLot-experemental: веб-версия с плеером, улучшенная загрузка картинок и правки в Rust-клиенте.
        </p>
        <a className="btn btn-primary mt-4" href="/patches/anilot-web-player.patch" download>
          <Download size={16} /> Скачать .patch
        </a>
      </section>
    </div>
  );
}
