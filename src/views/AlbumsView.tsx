import { useEffect, useState } from "react";
import { Disc3, Play } from "lucide-react";
import { useApp } from "../store";
import * as api from "../api";
import type { Album } from "../types";
import { Art } from "../components/Art";

export function AlbumsView() {
  const { t, navigate, libraryVersion, pushToast } = useApp();
  const [albums, setAlbums] = useState<Album[]>([]);

  useEffect(() => {
    api.getAlbums().then(setAlbums).catch(() => undefined);
  }, [libraryVersion]);

  return (
    <div className="view">
      <div className="view-header">
        <h1>{t("nav.albums")}</h1>
        <span className="view-count">
          {albums.length} {t("library.albums")}
        </span>
      </div>

      {albums.length === 0 ? (
        <div className="empty-state hero">
          <Disc3 size={52} strokeWidth={1.1} />
          <p>{t("emptyState.library")}</p>
        </div>
      ) : (
        <div className="grid albums-grid">
          {albums.map((album) => (
            <div
              key={album.id}
              className="card album-card"
              onClick={() => navigate({ name: "album", title: album.title, artist: album.artist })}
            >
              <div className="card-art">
                <Art hash={album.artHash} alt={album.title} className="card-img" />
                <button
                  className="card-play"
                  title={t("player.play")}
                  onClick={(e) => {
                    e.stopPropagation();
                    api.playAlbum(album.title, album.artist).catch((err) => pushToast(String(err), "error"));
                  }}
                >
                  <Play size={18} fill="currentColor" className="play-glyph" />
                </button>
              </div>
              <div className="card-meta">
                <span className="card-title">{album.title || t("library.unknown")}</span>
                <span className="card-sub">
                  {album.artist || t("library.unknown")}
                  {album.year ? ` • ${album.year}` : ""}
                </span>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
