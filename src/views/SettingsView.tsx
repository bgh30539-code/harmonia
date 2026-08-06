import { useEffect, useState } from "react";
import { FolderMinus, FolderPlus, Music2 } from "lucide-react";
import { useApp } from "../store";
import * as api from "../api";
import type { Folder, Settings } from "../types";
import { EQ_BANDS } from "../types";
import { applyTheme } from "../utils";

const ACCENTS = ["#7c5cff", "#0ea5e9", "#22c55e", "#f59e0b", "#ef4444", "#ec4899", "#14b8a6", "#a855f7"];
const THEMES: { key: Settings["theme"]; label: string }[] = [
  { key: "system", label: "settings.theme.system" },
  { key: "light", label: "settings.theme.light" },
  { key: "dark", label: "settings.theme.dark" },
];

export function SettingsView() {
  const { t, settings, pushToast } = useApp();
  const [draft, setDraft] = useState<Settings | null>(settings);
  const [folders, setFolders] = useState<Folder[]>([]);
  const [devices, setDevices] = useState<string[]>([]);

  useEffect(() => setDraft(settings), [settings]);

  useEffect(() => {
    api.listFolders().then(setFolders).catch(() => undefined);
    api.getAudioDevices().then(setDevices).catch(() => undefined);
  }, []);

  // Live-preview the visual settings (theme/accent) as the user edits them,
  // before they press Save. On teardown (navigating away) restore the values
  // that are actually persisted so an unsaved preview doesn't linger.
  useEffect(() => {
    if (!draft) return;
    const cleanup = applyTheme(draft.theme, draft.accent);
    return () => {
      cleanup();
      // Restore the persisted values so an unsaved preview doesn't linger.
      // Apply + immediately dispose: the store's own effect owns the long-lived
      // system-preference listener.
      applyTheme(settings?.theme ?? "system", settings?.accent ?? "#7c5cff")();
    };
  }, [draft?.theme, draft?.accent, settings?.theme, settings?.accent]);

  if (!draft) return null;

  const save = () => {
    api
      .updateSettings(draft)
      .then(() => pushToast(t("settings.saved"), "success"))
      .catch((e) => pushToast(String(e), "error"));
  };

  const patch = (p: Partial<Settings>) => {
    setDraft((d) => (d ? { ...d, ...p } : d));
    // Live-apply volume/speed so playback responds immediately.
    if (p.volume !== undefined) void api.setVolume(p.volume);
    if (p.shuffle !== undefined) void api.setShuffle(p.shuffle);
    if (p.repeat !== undefined) void api.setRepeat(p.repeat);
  };

  return (
    <div className="view settings-view">
      <div className="view-header">
        <h1>{t("settings.title")}</h1>
      </div>

      <section className="settings-section">
        <h2>{t("settings.appearance")}</h2>
        <div className="settings-grid">
          <div className="setting-row">
            <label>{t("settings.theme")}</label>
            <div className="segmented">
              {THEMES.map((th) => (
                <button
                  key={th.key}
                  className={`segment ${draft.theme === th.key ? "active" : ""}`}
                  onClick={() => patch({ theme: th.key })}
                >
                  {t(th.label)}
                </button>
              ))}
            </div>
          </div>
          <div className="setting-row">
            <label>{t("settings.accent")}</label>
            <div className="swatches">
              {ACCENTS.map((c) => (
                <button
                  key={c}
                  className={`swatch ${draft.accent === c ? "active" : ""}`}
                  style={{ background: c }}
                  onClick={() => patch({ accent: c })}
                  aria-label={c}
                />
              ))}
            </div>
          </div>
          <div className="setting-row">
            <label>{t("settings.language")}</label>
            <select
              className="select"
              value={draft.language}
              onChange={(e) => patch({ language: e.target.value })}
            >
              <option value="en">English</option>
              <option value="es">Español</option>
            </select>
          </div>
        </div>
      </section>

      <section className="settings-section">
        <h2>{t("settings.playback")}</h2>
        <div className="settings-grid">
          <div className="setting-row">
            <label>{t("settings.crossfade")}</label>
            <input
              type="range"
              min={0}
              max={12}
              step={0.5}
              value={draft.crossfadeSeconds}
              onChange={(e) => patch({ crossfadeSeconds: Number(e.target.value) })}
            />
            <span className="setting-value">{draft.crossfadeSeconds.toFixed(1)}s</span>
          </div>
          <div className="setting-row">
            <label>{t("settings.speed")}</label>
            <input
              type="range"
              min={0.5}
              max={2}
              step={0.05}
              value={draft.playbackSpeed}
              onChange={(e) => {
                const v = Number(e.target.value);
                patch({ playbackSpeed: v });
                void api.setSpeed(v);
              }}
            />
            <span className="setting-value">{draft.playbackSpeed.toFixed(2)}×</span>
          </div>
          <label className="setting-check">
            <input
              type="checkbox"
              checked={draft.replayGain}
              onChange={(e) => patch({ replayGain: e.target.checked })}
            />
            <span>{t("settings.replayGain")}</span>
          </label>
          <label className="setting-check">
            <input
              type="checkbox"
              checked={draft.resumeLastSession}
              onChange={(e) => patch({ resumeLastSession: e.target.checked })}
            />
            <span>{t("settings.resume")}</span>
          </label>
        </div>
      </section>

      <section className="settings-section">
        <h2>{t("settings.audio")}</h2>
        <div className="settings-grid">
          <div className="setting-row">
            <label>{t("settings.device")}</label>
            <select
              className="select"
              value={draft.audioDevice ?? ""}
              onChange={(e) => patch({ audioDevice: e.target.value || null })}
            >
              <option value="">{t("settings.device.default")}</option>
              {devices.map((d) => (
                <option key={d} value={d}>
                  {d}
                </option>
              ))}
            </select>
          </div>
          <label className="setting-check">
            <input
              type="checkbox"
              checked={draft.eqEnabled}
              onChange={(e) => patch({ eqEnabled: e.target.checked })}
            />
            <span>{t("settings.eq.enabled")}</span>
          </label>
          <div className="setting-row setting-eq">
            <label>{t("settings.eq")}</label>
            <div className="eq-sliders">
              {EQ_BANDS.map((hz, i) => (
                <div key={hz} className="eq-col">
                  <input
                    type="range"
                    min={-12}
                    max={12}
                    step={0.5}
                    value={draft.eqGains[i] ?? 0}
                    onChange={(e) => {
                      const gains = [...draft.eqGains];
                      gains[i] = Number(e.target.value);
                      patch({ eqGains: gains });
                    }}
                  />
                  <span className="eq-hz">{hz >= 1000 ? `${hz / 1000}k` : hz}</span>
                </div>
              ))}
            </div>
          </div>
          <div className="setting-row">
            <label>{t("settings.bassBoost")}</label>
            <input
              type="range"
              min={0}
              max={12}
              step={0.5}
              value={draft.bassBoostDb}
              onChange={(e) => patch({ bassBoostDb: Number(e.target.value) })}
            />
            <span className="setting-value">{draft.bassBoostDb.toFixed(1)} dB</span>
          </div>
          <div className="setting-row">
            <label>{t("settings.balance")}</label>
            <input
              type="range"
              min={-1}
              max={1}
              step={0.05}
              value={draft.balance}
              onChange={(e) => patch({ balance: Number(e.target.value) })}
            />
            <span className="setting-value">
              {draft.balance < -0.05 ? "L" : draft.balance > 0.05 ? "R" : "C"}
            </span>
          </div>
          <label className="setting-check">
            <input
              type="checkbox"
              checked={draft.mono}
              onChange={(e) => patch({ mono: e.target.checked })}
            />
            <span>{t("settings.mono")}</span>
          </label>
        </div>
      </section>

      <section className="settings-section">
        <h2>{t("settings.library")}</h2>
        <div className="settings-grid">
          <div className="setting-row folders-row">
            <label>{t("library.folders")}</label>
            <div className="folder-list">
              {folders.map((f) => (
                <div key={f.path} className="folder-item">
                  <span className="folder-path" title={f.path}>
                    {f.path}
                  </span>
                  <button
                    className="icon-btn danger"
                    title={t("close")}
                    onClick={() =>
                      void api.removeFolder(f.path).then(() => api.listFolders().then(setFolders))
                    }
                  >
                    <FolderMinus size={14} />
                  </button>
                </div>
              ))}
              {folders.length === 0 && <p className="dim">{t("library.empty.body")}</p>}
              <button
                className="btn btn-ghost"
                onClick={() =>
                  void api.addFolder().then((p) => {
                    if (p) {
                      api.listFolders().then(setFolders);
                      pushToast(t("library.folderAdded"), "success");
                    }
                  })
                }
              >
                <FolderPlus size={15} />
                <span>{t("library.addFolder")}</span>
              </button>
            </div>
          </div>
          <div className="setting-row">
            <label>{t("settings.cacheSize")}</label>
            <input
              type="range"
              min={64}
              max={2048}
              step={64}
              value={draft.cacheSizeMb}
              onChange={(e) => patch({ cacheSizeMb: Number(e.target.value) })}
            />
            <span className="setting-value">{draft.cacheSizeMb} MiB</span>
          </div>
        </div>
      </section>

      <section className="settings-section">
        <h2>{t("settings.about")}</h2>
        <div className="about-row">
          <Music2 size={20} />
          <span>
            {t("app.name")} — {t("settings.version", { version: "0.1.0" })}
          </span>
        </div>
      </section>

      <div className="settings-save">
        <button className="btn btn-primary" onClick={save}>
          {t("save")}
        </button>
      </div>
    </div>
  );
}
