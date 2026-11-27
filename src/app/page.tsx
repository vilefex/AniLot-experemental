export const dynamic = "force-dynamic";

const builds = [
  { os: "Linux", file: "AniLot-linux-x86_64.tar.gz / .tar.zst", note: "внутри ELF-бинарник anilot" },
  { os: "Windows", file: "AniLot-windows-x86_64.exe", note: "одиночный .exe" },
  { os: "macOS", file: "AniLot-macos-universal.dmg", note: "arm64 + x86_64, без подписи" },
  { os: "Android", file: "AniLot-android.apk", note: "arm64-v8a, armeabi-v7a, x86_64" },
  { os: "iOS", file: "AniLot.ipa", note: "без подписи — нужен сайдлоад" },
];

const features = [
  "Каталог, поиск с фильтрами, расписание, лента и коллекции Anixart",
  "Страница релиза: списки, избранное, оценки, комментарии, видео, галерея",
  "Выбор озвучки → источника → серии, запоминание источника и позиции",
  "Плеер: качество HLS, скорость, субтитры (VTT/SRT/ASS), mpv / VLC / Outplayer / nPlayer",
  "AniSkip: автопропуск опенингов и эндингов с кэшем меток",
  "Синхронизация со Shikimori в обе стороны с отчётом и остановкой",
  "Загрузки серий, проверка обновлений через GitHub Releases, светлая/тёмная тема и акцентный цвет",
];

export default function HomePage() {
  return (
    <main className="mx-auto min-h-screen max-w-3xl px-6 py-12 text-slate-100">
      <p className="text-sm uppercase tracking-[0.08em] text-orange-400">Rust + Iced</p>
      <h1 className="mt-3 text-5xl font-semibold text-white">AniLot</h1>
      <p className="mt-4 text-slate-300">
        Полный перенос неофициального клиента Anixart с Objective-C/UIKit на Rust и Iced. Исходники приложения лежат в
        каталоге <code className="rounded bg-slate-800 px-1.5 py-0.5">anilot/</code>, сборка — в{" "}
        <code className="rounded bg-slate-800 px-1.5 py-0.5">.github/workflows/anilot.yml</code>.
      </p>

      <h2 className="mt-10 text-xl font-semibold text-white">Артефакты workflow</h2>
      <ul className="mt-3 space-y-2">
        {builds.map((b) => (
          <li key={b.os} className="rounded-xl bg-slate-900 p-4">
            <span className="font-semibold text-orange-400">{b.os}</span>
            <span className="ml-3 font-mono text-sm text-slate-200">{b.file}</span>
            <div className="text-sm text-slate-400">{b.note}</div>
          </li>
        ))}
      </ul>

      <h2 className="mt-10 text-xl font-semibold text-white">Возможности</h2>
      <ul className="mt-3 list-disc space-y-1 pl-6 text-slate-300">
        {features.map((f) => (
          <li key={f}>{f}</li>
        ))}
      </ul>
    </main>
  );
}
