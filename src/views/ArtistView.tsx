import { useEffect, useState } from "react";
import { ArrowLeft, MicVocal, Play, Shuffle } from "lucide-react";
import { useApp } from "../store";
import * as api from "../api";
import type { Track } from "../types";
import { TrackTable } from "../components/TrackTable";

export function ArtistView({ name }: { name: string }) {
  const { t, navigate, pushToast } = useApp();
  const [tracks, setTracks] = useState<Track[]>([]);

  useEffect(() => {
    api.getArtistTracks(name).then(setTracks).catch(() => undefined);
  }, [name]);

  const albumCount = new Set(tracks.map((tr) => tr.album)).size;

  return (
    <div className="view">
      <button className="btn btn-ghost back-btn" onClick={() => navigate({ name: "artists" })}>
        <ArrowLeft size={15} />
        <span>{t("nav.artists")}</span>
      </button>

      <div className="detail-header">
        <div className="detail-avatar">
          <MicVocal size={40} strokeWidth={1.2} />
        </div>
        <div className="detail-meta">
          <h1>{name || t("library.unknown")}</h1>
          <p className="detail-sub dim">
            {albumCount} {t("library.albums")} • {tracks.length} {t("library.tracks")}
          </p>
          <div className="view-actions detail-actions">
            <button
              className="btn btn-primary"
              disabled={tracks.length === 0}
              onClick={() => void api.playArtist(name).catch((e) => pushToast(String(e), "error"))}
            >
              <Play size={15} fill="currentColor" className="play-glyph" />
              <span>{t("player.play")}</span>
            </button>
            <button
              className="btn btn-ghost"
              disabled={tracks.length === 0}
              onClick={() => void api.playArtist(name, true).catch((e) => pushToast(String(e), "error"))}
            >
              <Shuffle size={15} />
              <span>{t("player.shuffle")}</span>
            </button>
          </div>
        </div>
      </div>

      <TrackTable tracks={tracks} showArtist={false} showAlbum />
    </div>
  );
}
