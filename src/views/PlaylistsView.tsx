import { useEffect, useState } from "react";
import { ListMusic, Plus, Pin, Upload } from "lucide-react";
import { useApp } from "../store";
import * as api from "../api";
import type { Playlist } from "../types";
import { Modal } from "../components/Modal";

export function PlaylistsView() {
  const { t, navigate, libraryVersion, pushToast, openContextMenu } = useApp();
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const [creating, setCreating] = useState(false);
  const [creatingSmart, setCreatingSmart] = useState(false);
  const [name, setName] = useState("");

  const reload = () => api.listPlaylists().then(setPlaylists).catch(() => undefined);
  useEffect(() => {
    reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [libraryVersion]);

  const create = (kind: "static" | "smart") => {
    const trimmed = name.trim();
    if (!trimmed) return;
    api
      .createPlaylist(trimmed, kind)
      .then(() => {
        setCreating(false);
        setCreatingSmart(false);
        setName("");
        reload();
        pushToast(t("playlist.create") + " ✓", "success");
      })
      .catch((e) => pushToast(String(e), "error"));
  };

  const openPlaylistMenu = (e: React.MouseEvent, p: Playlist) => {
    e.preventDefault();
    openContextMenu(e.clientX, e.clientY, [
      {
        id: "pin",
        label: p.pinned ? t("playlist.unpin") : t("playlist.pin"),
        onSelect: () => void api.setPlaylistPinned(p.id, !p.pinned).then(reload),
      },
      {
        id: "rename",
        label: t("playlist.rename"),
        onSelect: () => {
          const next = window.prompt(t("playlist.name"), p.name);
          if (next) void api.renamePlaylist(p.id, next.trim()).then(reload);
        },
      },
      {
        id: "export",
        label: t("playlist.export"),
        onSelect: () => void api.exportPlaylist(p.id, "m3u").then(reload),
      },
      { id: "sep", label: "", separator: true },
      {
        id: "delete",
        label: t("playlist.delete"),
        danger: true,
        onSelect: () => void api.deletePlaylist(p.id).then(reload),
      },
    ]);
  };

  return (
    <div className="view">
      <div className="view-header">
        <h1>{t("nav.playlists")}</h1>
        <div className="view-actions">
          <button className="btn btn-ghost" onClick={() => void api.importPlaylist().then(reload)}>
            <Upload size={15} />
            <span>{t("playlist.import")}</span>
          </button>
          <button
            className="btn btn-ghost"
            onClick={() => {
              setName("");
              setCreatingSmart(true);
            }}
          >
            <Plus size={15} />
            <span>{t("playlist.smart")}</span>
          </button>
          <button
            className="btn btn-primary"
            onClick={() => {
              setName("");
              setCreating(true);
            }}
          >
            <Plus size={15} />
            <span>{t("playlist.new")}</span>
          </button>
        </div>
      </div>

      {playlists.length === 0 ? (
        <div className="empty-state hero">
          <ListMusic size={52} strokeWidth={1.1} />
          <p>{t("playlist.empty")}</p>
        </div>
      ) : (
        <div className="grid playlists-grid">
          {playlists.map((p) => (
            <div
              key={p.id}
              className="card playlist-card"
              onClick={() => navigate({ name: "playlist", id: p.id, title: p.name })}
              onContextMenu={(e) => openPlaylistMenu(e, p)}
            >
              <div className="playlist-icon">
                <ListMusic size={26} strokeWidth={1.4} />
                {p.pinned && <Pin size={13} className="pin-badge" fill="currentColor" />}
              </div>
              <div className="card-meta">
                <span className="card-title">{p.name}</span>
                <span className="card-sub">
                  {p.kind === "smart" ? t("playlist.smart") : ""}
                  {p.trackCount} {t("library.tracks")}
                </span>
              </div>
            </div>
          ))}
        </div>
      )}

      {(creating || creatingSmart) && (
        <Modal
          title={creatingSmart ? t("playlist.createSmart") : t("playlist.new")}
          onClose={() => {
            setCreating(false);
            setCreatingSmart(false);
          }}
          footer={
            <>
              <button
                className="btn btn-ghost"
                onClick={() => {
                  setCreating(false);
                  setCreatingSmart(false);
                }}
              >
                {t("cancel")}
              </button>
              <button className="btn btn-primary" onClick={() => create(creatingSmart ? "smart" : "static")}>
                {t("create")}
              </button>
            </>
          }
        >
          <input
            className="input"
            autoFocus
            placeholder={t("playlist.name")}
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") create(creatingSmart ? "smart" : "static");
            }}
          />
        </Modal>
      )}
    </div>
  );
}
