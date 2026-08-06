import { useEffect, useState } from "react";
import { ArrowLeft, ListMusic, Pin, Play, Shuffle, Upload } from "lucide-react";
import { useApp } from "../store";
import * as api from "../api";
import type { Playlist, SmartRules, Track } from "../types";
import { TrackTable } from "../components/TrackTable";
import { Modal } from "../components/Modal";

const FIELDS = ["title", "artist", "album", "genre", "year", "playCount", "durationMs", "composer"];
const OPS = ["=", "!=", "contains", "not contains", ">", "<"];

export function PlaylistView({ id, name }: { id: number; name: string }) {
  const { t, navigate, libraryVersion, pushToast } = useApp();
  const [tracks, setTracks] = useState<Track[]>([]);
  const [playlist, setPlaylist] = useState<Playlist | null>(null);
  const [editingRules, setEditingRules] = useState(false);
  const [rules, setRules] = useState<SmartRules>({ matchAll: true, rules: [] });

  const reload = () => {
    api.getPlaylistTracks(id).then(setTracks).catch(() => undefined);
    api.listPlaylists().then((pls) => setPlaylist(pls.find((p) => p.id === id) ?? null)).catch(() => undefined);
  };

  useEffect(() => {
    reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, libraryVersion]);

  const totalMs = tracks.reduce((acc, tr) => acc + tr.durationMs, 0);

  const saveRules = () => {
    api
      .updateSmartRules(id, JSON.stringify(rules))
      .then(() => {
        setEditingRules(false);
        reload();
        pushToast(t("settings.saved"), "success");
      })
      .catch((e) => pushToast(String(e), "error"));
  };

  return (
    <div className="view">
      <button className="btn btn-ghost back-btn" onClick={() => navigate({ name: "playlists" })}>
        <ArrowLeft size={15} />
        <span>{t("nav.playlists")}</span>
      </button>

      <div className="detail-header">
        <div className="playlist-icon large">
          <ListMusic size={34} strokeWidth={1.3} />
          {playlist?.pinned && <Pin size={14} className="pin-badge" fill="currentColor" />}
        </div>
        <div className="detail-meta">
          <h1>{name}</h1>
          <p className="detail-sub dim">
            {tracks.length} {t("library.tracks")} • {Math.round(totalMs / 60000)} min
            {playlist?.kind === "smart" ? ` • ${t("playlist.smart")}` : ""}
          </p>
          <div className="view-actions detail-actions">
            <button
              className="btn btn-primary"
              disabled={tracks.length === 0}
              onClick={() => void api.playPlaylist(id).catch((e) => pushToast(String(e), "error"))}
            >
              <Play size={15} fill="currentColor" className="play-glyph" />
              <span>{t("player.play")}</span>
            </button>
            <button
              className="btn btn-ghost"
              disabled={tracks.length === 0}
              onClick={() => void api.playPlaylist(id, true).catch((e) => pushToast(String(e), "error"))}
            >
              <Shuffle size={15} />
              <span>{t("player.shuffle")}</span>
            </button>
            {playlist?.kind === "smart" && (
              <button className="btn btn-ghost" onClick={() => setEditingRules(true)}>
                {t("playlist.smartRules")}
              </button>
            )}
            <button
              className="btn btn-ghost"
              onClick={() => void api.exportPlaylist(id, "m3u").catch((e) => pushToast(String(e), "error"))}
            >
              <Upload size={15} />
              <span>{t("playlist.export")}</span>
            </button>
          </div>
        </div>
      </div>

      <TrackTable
        tracks={tracks}
        playlistId={id}
        onReorder={(orderedIds) =>
          void api.reorderPlaylist(id, orderedIds).catch((e) => pushToast(String(e), "error"))
        }
        onRemoveFromPlaylist={(trackId) =>
          void api.removeTrackFromPlaylist(id, trackId).then(reload).catch((e) => pushToast(String(e), "error"))
        }
        emptyMessage={t("playlist.empty")}
      />

      {editingRules && (
        <Modal
          title={t("playlist.smartRules")}
          onClose={() => setEditingRules(false)}
          footer={
            <>
              <button className="btn btn-ghost" onClick={() => setEditingRules(false)}>
                {t("cancel")}
              </button>
              <button className="btn btn-primary" onClick={saveRules}>
                {t("save")}
              </button>
            </>
          }
        >
          <div className="rules-editor">
            <label className="rules-match">
              <input
                type="checkbox"
                checked={rules.matchAll}
                onChange={(e) => setRules({ ...rules, matchAll: e.target.checked })}
              />
              <span>{t("playlist.smart")} (match all)</span>
            </label>
            {rules.rules.map((rule, i) => (
              <div key={i} className="rule-row">
                <select
                  className="select"
                  value={rule.field}
                  onChange={(e) => {
                    const next = [...rules.rules];
                    next[i] = { ...rule, field: e.target.value };
                    setRules({ ...rules, rules: next });
                  }}
                >
                  {FIELDS.map((f) => (
                    <option key={f} value={f}>
                      {f}
                    </option>
                  ))}
                </select>
                <select
                  className="select"
                  value={rule.op}
                  onChange={(e) => {
                    const next = [...rules.rules];
                    next[i] = { ...rule, op: e.target.value };
                    setRules({ ...rules, rules: next });
                  }}
                >
                  {OPS.map((o) => (
                    <option key={o} value={o}>
                      {o}
                    </option>
                  ))}
                </select>
                <input
                  className="input"
                  value={rule.value}
                  onChange={(e) => {
                    const next = [...rules.rules];
                    next[i] = { ...rule, value: e.target.value };
                    setRules({ ...rules, rules: next });
                  }}
                />
                <button
                  className="icon-btn danger"
                  onClick={() =>
                    setRules({ ...rules, rules: rules.rules.filter((_, j) => j !== i) })
                  }
                >
                  ✕
                </button>
              </div>
            ))}
            <button
              className="btn btn-ghost"
              onClick={() => setRules({ ...rules, rules: [...rules.rules, { field: "title", op: "contains", value: "" }] })}
            >
              + {t("playlist.smartRules")}
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}
