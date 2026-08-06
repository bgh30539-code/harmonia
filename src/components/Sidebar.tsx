import { useEffect, useState } from "react";
import {
  AudioLines,
  Clock,
  Disc3,
  Heart,
  Library,
  ListMusic,
  MicVocal,
  Settings,
  Star,
} from "lucide-react";
import { useApp, type View } from "../store";
import * as api from "../api";
import type { Playlist } from "../types";

interface NavItem {
  key: string;
  label: string;
  icon: React.ReactNode;
  view: View;
}

export function Sidebar() {
  const { t, view, navigate, libraryVersion } = useApp();
  const [playlists, setPlaylists] = useState<Playlist[]>([]);

  useEffect(() => {
    api.listPlaylists().then(setPlaylists).catch(() => undefined);
  }, [libraryVersion]);

  const items: NavItem[] = [
    { key: "library", label: t("nav.library"), icon: <Library size={18} />, view: { name: "library" } },
    { key: "albums", label: t("nav.albums"), icon: <Disc3 size={18} />, view: { name: "albums" } },
    { key: "artists", label: t("nav.artists"), icon: <MicVocal size={18} />, view: { name: "artists" } },
    { key: "playlists", label: t("nav.playlists"), icon: <ListMusic size={18} />, view: { name: "playlists" } },
    { key: "favorites", label: t("nav.favorites"), icon: <Heart size={18} />, view: { name: "favorites" } },
    { key: "recent", label: t("nav.recent"), icon: <Clock size={18} />, view: { name: "recent" } },
    { key: "mostPlayed", label: t("nav.mostPlayed"), icon: <Star size={18} />, view: { name: "mostPlayed" } },
  ];

  const pinned = playlists.filter((p) => p.pinned);
  const isActive = (v: View) => v.name === view.name;

  return (
    <aside className="sidebar">
      <div className="brand" onClick={() => navigate({ name: "library" })}>
        <span className="brand-mark">
          <AudioLines size={20} />
        </span>
        <span className="brand-name">{t("app.name")}</span>
      </div>

      <nav className="nav">
        {items.map((item) => (
          <button
            key={item.key}
            className={`nav-item ${isActive(item.view) ? "active" : ""}`}
            onClick={() => navigate(item.view)}
          >
            {item.icon}
            <span>{item.label}</span>
          </button>
        ))}

        {pinned.length > 0 && (
          <>
            <div className="nav-section-label">{t("nav.pinned")}</div>
            {pinned.map((p) => (
              <button
                key={p.id}
                className={`nav-item ${view.name === "playlist" && view.id === p.id ? "active" : ""}`}
                onClick={() => navigate({ name: "playlist", id: p.id, title: p.name })}
              >
                <ListMusic size={18} />
                <span className="nav-label-ellipsis">{p.name}</span>
              </button>
            ))}
          </>
        )}
      </nav>

      <div className="sidebar-footer">
        <button
          className={`nav-item ${view.name === "settings" ? "active" : ""}`}
          onClick={() => navigate({ name: "settings" })}
        >
          <Settings size={18} />
          <span>{t("nav.settings")}</span>
        </button>
      </div>
    </aside>
  );
}
