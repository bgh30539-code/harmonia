import { Maximize2, Pause, Play, SkipBack, SkipForward, X } from "lucide-react";
import { useApp } from "../store";
import * as api from "../api";
import { formatDuration, clamp } from "../utils";
import { Art } from "./Art";

export function MiniPlayer() {
  const { playback, positionMs, t, pushToast } = useApp();
  const track = playback?.current ?? null;
  const playing = playback?.playing ?? false;
  const durationMs = playback?.durationMs ?? track?.durationMs ?? 0;
  const progress = durationMs > 0 ? clamp(positionMs / durationMs, 0, 1) : 0;

  const exit = () => {
    api.setMiniPlayer(false).catch((e) => pushToast(String(e), "error"));
  };

  return (
    <div className="mini-player">
      <Art hash={track?.artHash ?? null} alt={track?.title} className="mini-art" />
      <div className="mini-meta">
        <span className="mini-title">{track?.title ?? t("player.nowPlaying")}</span>
        <span className="mini-sub">{track?.artist || t("library.unknown")}</span>
        <div className="mini-progress">
          <span className="mini-fill" style={{ width: `${progress * 100}%` }} />
        </div>
      </div>
      <div className="mini-controls">
        <button className="icon-btn" title={t("player.previous")} onClick={() => void api.playPrevious()}>
          <SkipBack size={16} fill="currentColor" />
        </button>
        <button className="icon-btn mini-play" title={playing ? t("player.pause") : t("player.play")} onClick={() => void api.togglePlayback()}>
          {playing ? <Pause size={18} fill="currentColor" /> : <Play size={18} fill="currentColor" className="play-glyph" />}
        </button>
        <button className="icon-btn" title={t("player.next")} onClick={() => void api.playNext()}>
          <SkipForward size={16} fill="currentColor" />
        </button>
        <button className="icon-btn" title={t("player.miniPlayer")} onClick={exit}>
          <Maximize2 size={15} />
        </button>
      </div>
      <span className="mini-time">{formatDuration(positionMs)}</span>
      <button className="icon-btn mini-close" onClick={exit}>
        <X size={14} />
      </button>
    </div>
  );
}
