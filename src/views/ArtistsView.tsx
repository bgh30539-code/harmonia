import { useEffect, useState } from "react";
import { MicVocal, Play } from "lucide-react";
import { useApp } from "../store";
import * as api from "../api";
import type { Artist } from "../types";
import { Art } from "../components/Art";

export function ArtistsView() {
  const { t, navigate, libraryVersion, pushToast } = useApp();
  const [artists, setArtists] = useState<Artist[]>([]);

  useEffect(() => {
    api.getArtists().then(setArtists).catch(() => undefined);
  }, [libraryVersion]);

  return (
    <div className="view">
      <div className="view-header">
        <h1>{t("nav.artists")}</h1>
        <span className="view-count">
          {artists.length} {t("library.artists")}
        </span>
      </div>

      {artists.length === 0 ? (
        <div className="empty-state hero">
          <MicVocal size={52} strokeWidth={1.1} />
          <p>{t("emptyState.library")}</p>
        </div>
      ) : (
        <div className="artists-list">
          {artists.map((artist) => (
            <div
              key={artist.id}
              className="artist-row"
              onClick={() => navigate({ name: "artist", artist: artist.name })}
            >
              <Art hash={artist.artHash} alt={artist.name} className="artist-avatar" />
              <div className="artist-meta">
                <span className="artist-name">{artist.name || t("library.unknown")}</span>
                <span className="artist-sub">
                  {artist.albumCount} {t("library.albums")} • {artist.trackCount} {t("library.tracks")}
                </span>
              </div>
              <button
                className="btn btn-ghost"
                title={t("player.play")}
                onClick={(e) => {
                  e.stopPropagation();
                  api.playArtist(artist.name).catch((err) => pushToast(String(err), "error"));
                }}
              >
                <Play size={15} fill="currentColor" className="play-glyph" />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
