"use client";

import { useSyncExternalStore } from "react";

// ───────────── типы ─────────────

/* eslint-disable @typescript-eslint/no-explicit-any */
export type Release = Record<string, any> & { id: number };

export type Settings = {
  conn: "auto" | "alt";
  custom: string;
  img: "auto" | "direct" | "proxy" | "alt";
  autoSkip: boolean;
  autoNext: boolean;
  speed: number;
  quality: number; // 0 = максимальное
  volume: number;
};

export const DEFAULT_SETTINGS: Settings = {
  conn: "auto",
  custom: "",
  img: "auto",
  autoSkip: true,
  autoNext: true,
  speed: 1,
  quality: 0,
  volume: 1,
};

// ───────────── localStorage-хранилище с подпиской ─────────────

const listeners = new Set<() => void>();
const cache = new Map<string, { raw: string | null; value: unknown }>();

function emit() {
  listeners.forEach((l) => l());
}

function subscribe(cb: () => void) {
  listeners.add(cb);
  const onStorage = () => cb();
  window.addEventListener("storage", onStorage);
  return () => {
    listeners.delete(cb);
    window.removeEventListener("storage", onStorage);
  };
}

function readStore<T>(key: string, fallback: T): T {
  if (typeof window === "undefined") return fallback;
  let raw: string | null = null;
  try {
    raw = localStorage.getItem(key);
  } catch {}
  const c = cache.get(key);
  if (c && c.raw === raw) return c.value as T;
  let value: T = fallback;
  if (raw != null) {
    try {
      value = { ...(typeof fallback === "object" && fallback ? fallback : {}), ...JSON.parse(raw) } as T;
      if (!(typeof fallback === "object" && fallback && !Array.isArray(fallback))) value = JSON.parse(raw) as T;
    } catch {
      value = fallback;
    }
  }
  cache.set(key, { raw, value });
  return value;
}

function writeStore(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {}
  emit();
}

export function getSettings(): Settings {
  return readStore("anilot.settings", DEFAULT_SETTINGS);
}

export function useSettings(): [Settings, (p: Partial<Settings>) => void] {
  const s = useSyncExternalStore(subscribe, () => readStore("anilot.settings", DEFAULT_SETTINGS), () => DEFAULT_SETTINGS);
  return [s, (p) => writeStore("anilot.settings", { ...getSettings(), ...p })];
}

const TOKEN_KEY = "anilot.token";
const EMPTY_AUTH = { token: "", login: "", id: 0, avatar: "" };
export type Auth = typeof EMPTY_AUTH;

export function getAuth(): Auth {
  return readStore(TOKEN_KEY, EMPTY_AUTH);
}
export function setAuth(a: Auth | null) {
  if (!a) {
    try {
      localStorage.removeItem(TOKEN_KEY);
    } catch {}
    emit();
  } else writeStore(TOKEN_KEY, a);
}
export function useAuth(): Auth {
  return useSyncExternalStore(subscribe, () => readStore(TOKEN_KEY, EMPTY_AUTH), () => EMPTY_AUTH);
}

// ───────────── прогресс просмотра ─────────────

export type Progress = {
  release: number;
  title: string;
  image: string;
  type: number;
  source: number;
  position: number;
  epName: string;
  time: number;
  duration: number;
  at: number;
};

const PROGRESS_KEY = "anilot.progress";
const EMPTY_PROGRESS: Record<string, Progress> = {};

export function useProgress(): Record<string, Progress> {
  return useSyncExternalStore(subscribe, () => readStore(PROGRESS_KEY, EMPTY_PROGRESS), () => EMPTY_PROGRESS);
}
export function saveProgress(p: Progress) {
  const all = { ...readStore(PROGRESS_KEY, EMPTY_PROGRESS) };
  all[String(p.release)] = p;
  const keys = Object.keys(all).sort((a, b) => all[b].at - all[a].at);
  for (const k of keys.slice(60)) delete all[k];
  writeStore(PROGRESS_KEY, all);
}

const EP_KEY = "anilot.eptime";
const EMPTY_EP: Record<string, { t: number; d: number; done?: boolean }> = {};
export function useEpisodeTimes() {
  return useSyncExternalStore(subscribe, () => readStore(EP_KEY, EMPTY_EP), () => EMPTY_EP);
}
export function saveEpisodeTime(release: number, source: number, position: number, t: number, d: number, done = false) {
  const all = { ...readStore(EP_KEY, EMPTY_EP) };
  all[`${release}:${source}:${position}`] = { t, d, done: done || (all[`${release}:${source}:${position}`]?.done ?? false) };
  const keys = Object.keys(all);
  if (keys.length > 1500) for (const k of keys.slice(0, keys.length - 1500)) delete all[k];
  writeStore(EP_KEY, all);
}
export function getEpisodeTime(release: number, source: number, position: number) {
  return readStore(EP_KEY, EMPTY_EP)[`${release}:${source}:${position}`];
}

// ───────────── API ─────────────

export class ApiError extends Error {}

type Opts = { method?: string; json?: unknown; form?: Record<string, string>; query?: Record<string, string | number>; auth?: boolean };

export async function ax<T = any>(path: string, o: Opts = {}): Promise<T> {
  const s = getSettings();
  const { token } = getAuth();
  const qs = new URLSearchParams();
  if (token && o.auth !== false) qs.set("token", token);
  for (const [k, v] of Object.entries(o.query || {})) qs.set(k, String(v));
  const headers: Record<string, string> = { "x-ax-mode": s.conn };
  if (s.custom.trim()) headers["x-ax-custom"] = s.custom.trim();
  let body: BodyInit | undefined;
  if (o.json !== undefined) {
    headers["content-type"] = "application/json; charset=UTF-8";
    body = JSON.stringify(o.json);
  } else if (o.form) {
    headers["content-type"] = "application/x-www-form-urlencoded; charset=UTF-8";
    body = new URLSearchParams(o.form).toString();
  }
  const method = o.method || (body ? "POST" : "GET");
  let res: Response;
  try {
    res = await fetch(`/api/ax/${path}${qs.toString() ? `?${qs}` : ""}`, { method, headers, body });
  } catch {
    throw new ApiError("Нет соединения с сервером");
  }
  let data: any;
  try {
    data = await res.json();
  } catch {
    throw new ApiError(`Некорректный ответ (HTTP ${res.status})`);
  }
  if (!res.ok && data?.error) throw new ApiError(data.error);
  return data as T;
}

export const EMPTY_FILTER = {
  sort: 0,
  category_id: null as number | null,
  status_id: null as number | null,
  season: null as number | null,
  start_year: null as number | null,
  end_year: null as number | null,
  genres: [] as string[],
  is_genres_exclude_mode_enabled: false,
  profile_list_exclusions: [] as number[],
  types: [] as number[],
  age_ratings: [] as number[],
  episodes_from: null as number | null,
  episodes_to: null as number | null,
};

export const SORTS = [
  { id: 0, name: "Обновления" },
  { id: 1, name: "Новые" },
  { id: 3, name: "Популярные" },
  { id: 4, name: "Рейтинг" },
  { id: 5, name: "Просмотры" },
];

export const GENRES = [
  "боевик", "комедия", "драма", "фэнтези", "приключения", "романтика", "школа", "сёнэн", "сэйнэн", "повседневность",
  "научная фантастика", "мистика", "психологическое", "триллер", "спорт", "меха", "магия", "детектив", "ужасы", "музыка",
];

export const LIST_STATUSES = [
  { id: 1, name: "Смотрю" },
  { id: 2, name: "В планах" },
  { id: 3, name: "Просмотрено" },
  { id: 4, name: "Отложено" },
  { id: 5, name: "Брошено" },
];

export function title(r: Release): string {
  return r.title_ru || r.title_original || `#${r.id}`;
}

export function fmtTime(sec: number): string {
  if (!Number.isFinite(sec) || sec < 0) sec = 0;
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = Math.floor(sec % 60);
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}` : `${m}:${String(s).padStart(2, "0")}`;
}

export function episodeNumber(name: string, fallback: number): number {
  const m = (name || "").match(/(\d{1,4})/);
  return m ? Number(m[1]) : fallback;
}
