export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return `${m}:${String(s).padStart(2, "0")}`;
}

export function formatRemaining(ms: number): string {
  return `-${formatDuration(ms)}`;
}

export function formatTotalDuration(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  if (h > 0) return `${h} h ${m} min`;
  return `${m} min`;
}

export function formatDate(ts: number | null): string {
  if (!ts) return "—";
  return new Date(ts * 1000).toLocaleDateString();
}

export function formatBitrate(b: number | null): string {
  if (!b) return "—";
  return `${b} kbps`;
}

export function formatHz(hz: number | null): string {
  if (!hz) return "—";
  return hz >= 1000 ? `${(hz / 1000).toFixed(1)} kHz` : `${hz} Hz`;
}

export function formatChannels(ch: number | null): string {
  if (!ch) return "—";
  if (ch === 1) return "Mono";
  if (ch === 2) return "Stereo";
  return `${ch} channels`;
}

/** "1:05:03" style elapsed time for queue rows. */
export function clamp(v: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, v));
}

/**
 * Apply the theme + accent to the document root. Returns a cleanup that
 * removes the system-preference listener (call it on effect teardown).
 */
export function applyTheme(theme: string, accent: string): () => void {
  const root = document.documentElement;
  const apply = () => {
    const dark =
      theme === "dark" ||
      (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
    root.dataset.theme = dark ? "dark" : "light";
  };
  apply();
  root.style.setProperty("--accent", accent);
  if (theme === "system") {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }
  return () => undefined;
}
