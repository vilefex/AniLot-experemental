"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { LogOut, UserRound } from "lucide-react";
import { ax, setAuth, useAuth } from "@/lib/client";

export default function LoginPage() {
  const auth = useAuth();
  const router = useRouter();
  const [login, setLogin] = useState("");
  const [password, setPassword] = useState("");
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setErr("");
    try {
      const d = await ax("auth/signIn", { form: { login, password }, auth: false });
      if (d.code === 2) throw new Error("Неверный логин");
      if (d.code === 3) throw new Error("Неверный пароль");
      const token = d.profileToken?.token;
      if (d.code !== 0 || !token) throw new Error(`Не удалось войти (код ${d.code})`);
      setAuth({ token, login: d.profile?.login || login, id: d.profile?.id || 0, avatar: d.profile?.avatar || "" });
      router.push("/library");
    } catch (e2) {
      setErr(e2 instanceof Error ? e2.message : "Ошибка входа");
    } finally {
      setBusy(false);
    }
  };

  if (auth.token) {
    return (
      <div className="mx-auto mt-16 max-w-md rounded-2xl border border-line bg-surface p-8 text-center">
        <div className="mx-auto grid h-16 w-16 place-items-center rounded-full bg-accent-soft text-accent">
          <UserRound size={30} />
        </div>
        <h1 className="mt-4 text-xl font-bold">{auth.login}</h1>
        <p className="mt-1 text-sm text-white/50">Вы вошли в аккаунт Anixart</p>
        <button className="btn mt-6" onClick={() => setAuth(null)}>
          <LogOut size={16} /> Выйти
        </button>
      </div>
    );
  }

  return (
    <form onSubmit={submit} className="mx-auto mt-16 max-w-sm space-y-4 rounded-2xl border border-line bg-surface p-8">
      <h1 className="text-xl font-bold">Вход в Anixart</h1>
      <input className="input" placeholder="Логин" value={login} onChange={(e) => setLogin(e.target.value)} autoComplete="username" required />
      <input
        className="input"
        type="password"
        placeholder="Пароль"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        autoComplete="current-password"
        required
      />
      {err && <div className="text-sm text-red-300">{err}</div>}
      <button className="btn btn-primary w-full" disabled={busy}>
        {busy ? "Входим…" : "Войти"}
      </button>
      <p className="text-xs text-white/40">Токен хранится только в вашем браузере. Просмотр без входа тоже работает.</p>
    </form>
  );
}
