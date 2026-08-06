import { useEffect, useState } from "react";
import { FileAudio, Heart, X } from "lucide-react";
import { useApp } from "../store";
import * as api from "../api";
import type { LyricsData } from "../types";
import { formatBitrate, formatChannels, formatDuration, formatHz } from "../utils";
import { Art } from "./Art";

/** A deterministic, playing-state-driven bar visualizer (per-track seed). */
function Visualizer({ playing, seed, progress }: { playing: boolean; seed: number; progress: number }) {
  const BARS = 44;
  const bars = Array.from({ length: BARS }, (_, i) => {
    // Deterministic pseudo-random heights so each track looks distinct.
    const h = ((Math.sin((i + 1) * 12.9898 + seed * 78.233) * 43758.5453) % 1 + 1) % 1;
    return 0.25 + h * 0.75;
  });
  return (
    <div className={`visualizer ${playing ? "playing" : ""}`} aria-hidden="true">
      {bars.map((h, i) => {
        const passed = i / BARS <= progress;
        return (
          <span
            key={i}
            className={`viz-bar ${passed ? "passed" : ""}`}
            style={{
              height: `${h * 100}%`,
              animationDelay: `${((i * 7) % 29) / 20}s`,
            }}
          />
        );
      })}
    </div>
  );
}

export function NowPlayingDrawer() {
  const { playback, positionMs, t, nowPlayingOpen, setNowPlayingOpen } = useApp();
  const [lyrics, setLyrics] = useState<LyricsData | null>(null);
  const [lyricsTrackId, setLyricsTrackId] = useState<number | null>(null);

  const track = playback?.current ?? null;
  const playing = playback?.playing ?? false;
  const durationMs = playback?.durationMs ?? track?.durationMs ?? 0;
  const progress = durationMs > 0 ? Math.min(1, positionMs / durationMs) : 0;

  useEffect(() => {
    if (!nowPlayingOpen || !track) return;
    if (lyricsTrackId === track.id) return;
    setLyricsTrackId(track.id);
    setLyrics(null);
    api
      .getLyrics(track.id)
      .then(setLyrics)
      .catch(() => setLyrics(null));
  }, [nowPlayingOpen, track, lyricsTrackId]);

  if (!nowPlayingOpen) return null;

  return (
    <aside className="drawer nowplaying-drawer">
      <div className="drawer-header">
        <h3>{t("player.nowPlaying")}</h3>
        <button className="icon-btn" onClick={() => setNowPlayingOpen(false)}>
          <X size={16} />
        </button>
      </div>

      <div className="np-body">
        <div className={`np-art-wrap ${playing ? "spinning" : ""}`}>
          <Art hash={track?.artHash ?? null} alt={track?.title} className="np-art" />
          <span className="np-disc-hole" />
        </div>

        <div className="np-meta">
          <span className="np-title">{track?.title || t("library.unknown")}</span>
          <span className="np-artist">{track?.artist || t("library.unknown")}</span>
          <span className="np-album">{track?.album || t("library.unknown")}</span>
          {track && (
            <button
              className={`np-fav ${track.favorite ? "faved" : ""}`}
              onClick={() => {
                const next = !track.favorite;
                void api.setFavorite(track.id, next);
                track.favorite = next;
              }}
            >
              <Heart size={16} fill={track.favorite ? "currentColor" : "none"} />
              {track.favorite ? t("context.unfavorite") : t("context.favorite")}
            </button>
          )}
        </div>

        <Visualizer playing={playing} seed={track?.id ?? 0} progress={progress} />

        <div className="np-progress">
          <span>{formatDuration(positionMs)}</span>
          <span>{formatDuration(durationMs)}</span>
        </div>

        {track && (
          <dl className="np-details">
            <div>
              <dt>{t("codec")}</dt>
              <dd>{track.format || "—"}</dd>
            </div>
            <div>
              <dt>{t("bitrate")}</dt>
              <dd>{formatBitrate(track.bitrate)}</dd>
            </div>
            <div>
              <dt>{t("sampleRate")}</dt>
              <dd>{formatHz(track.sampleRate)}</dd>
            </div>
            <div>
              <dt>{t("channels")}</dt>
              <dd>{formatChannels(track.channels)}</dd>
            </div>
            <div className="np-path">
              <dt>
                <FileAudio size={13} />
                {t("filePath")}
              </dt>
              <dd title={track.path}>{track.path}</dd>
            </div>
          </dl>
        )}

        <div className="np-lyrics">
          <h4>{t("player.lyrics")}</h4>
          {!track ? (
            <p className="np-lyrics-empty">{t("player.noLyrics")}</p>
          ) : lyrics === null ? (
            <p className="np-lyrics-loading">…</p>
          ) : lyrics.plain || lyrics.synced ? (
            <pre className="np-lyrics-text">{lyrics.plain ?? lyrics.synced}</pre>
          ) : (
            <p className="np-lyrics-empty">{t("player.noLyrics")}</p>
          )}
        </div>
      </div>
    </aside>
  );
}
