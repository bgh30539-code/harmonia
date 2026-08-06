import { useEffect, useState } from "react";
import { ArrowLeft, Play, Shuffle } from "lucide-react";
import { useApp } from "../store";
import * as api from "../api";
import type { Track } from "../types";
import { Art } from "../components/Art";
import { TrackTable } from "../components/TrackTable";
import { formatTotalDuration } from "../utils";

export function AlbumView({ title, artist }: { title: string; artist: string }) {
  const { t, navigate, pushToast } = useApp();
  const [tracks, setTracks] = useState<Track[]>([]);

  useEffect(() => {
    api.getAlbumTracks(title, artist).then(setTracks).catch(() => undefined);
  }, [title, artist]);

  const album = tracks[0]?.album ?? title;
  const albumArtist = tracks[0]?.albumArtist || tracks[0]?.artist || artist;
  const artHash = tracks[0]?.artHash ?? null;
  const year = tracks.find((tr) => tr.year)?.year ?? null;
  const totalMs = tracks.reduce((acc, tr) => acc + tr.durationMs, 0);

  return (
    <div className="view">
      <button className="btn btn-ghost back-btn" onClick={() => navigate({ name: "albums" })}>
        <ArrowLeft size={15} />
        <span>{t("nav.albums")}</span>
      </button>

      <div className="detail-header">
        <Art hash={artHash} alt={album} className="detail-art" />
        <div className="detail-meta">
          <h1>{album || t("library.unknown")}</h1>
          <p className="detail-sub">
            {albumArtist || t("library.unknown")}
            {year ? ` • ${year}` : ""}
          </p>
          <p className="detail-sub dim">
            {tracks.length} {t("library.tracks")} • {formatTotalDuration(totalMs)}
          </p>
          <div className="view-actions detail-actions">
            <button
              className="btn btn-primary"
              disabled={tracks.length === 0}
              onClick={() => void api.playAlbum(title, artist).catch((e) => pushToast(String(e), "error"))}
            >
              <Play size={15} fill="currentColor" className="play-glyph" />
              <span>{t("player.play")}</span>
            </button>
            <button
              className="btn btn-ghost"
              disabled={tracks.length === 0}
              onClick={() => void api.playAlbum(title, artist, true).catch((e) => pushToast(String(e), "error"))}
            >
              <Shuffle size={15} />
              <span>{t("player.shuffle")}</span>
            </button>
          </div>
        </div>
      </div>

      <TrackTable tracks={tracks} showArtist showAlbum={false} />
    </div>
  );
}
