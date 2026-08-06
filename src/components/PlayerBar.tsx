import { useRef, useState } from "react";
import {
  Heart,
  ListMusic,
  Maximize2,
  Pause,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  Timer,
  Volume1,
  Volume2,
  VolumeX,
} from "lucide-react";
import { useApp } from "../store";
import * as api from "../api";
import { clamp, formatDuration, formatRemaining } from "../utils";
import { Art } from "./Art";
import type { RepeatMode } from "../types";

function RepeatIcon({ mode }: { mode: RepeatMode }) {
  if (mode === "one") return <Repeat1 size={18} />;
  return <Repeat size={18} />;
}

export function PlayerBar() {
  const { playback, positionMs, t, queueOpen, setQueueOpen, setNowPlayingOpen, pushToast } = useApp();
  const [volumeOpen, setVolumeOpen] = useState(false);
  const [timerOpen, setTimerOpen] = useState(false);
  const barRef = useRef<HTMLDivElement>(null);
  const timerRef = useRef<HTMLDivElement>(null);

  const track = playback?.current ?? null;
  const playing = playback?.playing ?? false;
  const durationMs = playback?.durationMs ?? track?.durationMs ?? 0;
  const volume = playback?.volume ?? 0.9;
  const shuffle = playback?.shuffle ?? false;
  const repeat = playback?.repeat ?? "off";
  const sleep = playback?.sleepTimer ?? { kind: "off" as const, remainingSecs: null };

  const progress = durationMs > 0 ? clamp(positionMs / durationMs, 0, 1) : 0;

  const seekFromEvent = (e: React.PointerEvent) => {
    const el = barRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const frac = clamp((e.clientX - rect.left) / rect.width, 0, 1);
    void api.seek(Math.round(frac * durationMs));
  };

  const cycleRepeat = () => {
    const next: RepeatMode = repeat === "off" ? "all" : repeat === "all" ? "one" : "off";
    void api.setRepeat(next);
  };

  const VolumeIcon = volume <= 0 ? VolumeX : volume < 0.55 ? Volume1 : Volume2;
  const sleepActive = sleep.kind !== "off";

  // Canonical key describing the active sleep timer, for highlighting the selected option.
  const sleepKey =
    sleep.kind === "minutes" && sleep.remainingSecs !== null
      ? `m${Math.round(sleep.remainingSecs / 60)}`
      : sleep.kind;
  const timerOptions: { key: string; kind: "minutes" | "endOfTrack" | "endOfAlbum"; label: string; minutes?: number }[] = [
    { key: "m15", kind: "minutes", minutes: 15, label: t("player.sleepMinutes", { minutes: 15 }) },
    { key: "m30", kind: "minutes", minutes: 30, label: t("player.sleepMinutes", { minutes: 30 }) },
    { key: "m60", kind: "minutes", minutes: 60, label: t("player.sleepMinutes", { minutes: 60 }) },
    { key: "m90", kind: "minutes", minutes: 90, label: t("player.sleepMinutes", { minutes: 90 }) },
    { key: "endOfTrack", kind: "endOfTrack", label: t("player.sleepEndOfTrack") },
    { key: "endOfAlbum", kind: "endOfAlbum", label: t("player.sleepEndOfAlbum") },
  ];

  return (
    <footer className="player-bar">
      {/* Current track */}
      <div className="pb-track" onClick={() => setNowPlayingOpen(true)}>
        <Art hash={track?.artHash ?? null} alt={track?.title} className="pb-art" />
        <div className="pb-track-meta">
          <span className="pb-title">{track?.title ?? t("player.nowPlaying")}</span>
          <span className="pb-artist">{track?.artist || t("library.unknown")}</span>
        </div>
        {track && (
          <button
            className={`pb-fav ${track.favorite ? "faved" : ""}`}
            title={track.favorite ? t("context.unfavorite") : t("context.favorite")}
            onClick={(e) => {
              e.stopPropagation();
              const next = !track.favorite;
              void api.setFavorite(track.id, next);
              track.favorite = next;
            }}
          >
            <Heart size={15} fill={track.favorite ? "currentColor" : "none"} />
          </button>
        )}
      </div>

      {/* Transport */}
      <div className="pb-center">
        <div className="pb-transport">
          <button
            className={`icon-btn ${shuffle ? "active" : ""}`}
            title={t("player.shuffle")}
            onClick={() => void api.setShuffle(!shuffle)}
          >
            <Shuffle size={17} />
          </button>
          <button className="icon-btn" title={t("player.previous")} onClick={() => void api.playPrevious()}>
            <SkipBack size={19} fill="currentColor" />
          </button>
          <button
            className="icon-btn play-btn"
            title={playing ? t("player.pause") : t("player.play")}
            onClick={() => void api.togglePlayback()}
          >
            {playing ? (
              <Pause size={22} fill="currentColor" />
            ) : (
              <Play size={22} fill="currentColor" className="play-glyph" />
            )}
          </button>
          <button className="icon-btn" title={t("player.next")} onClick={() => void api.playNext()}>
            <SkipForward size={19} fill="currentColor" />
          </button>
          <button
            className={`icon-btn ${repeat !== "off" ? "active" : ""}`}
            title={
              repeat === "one" ? t("player.repeatOne") : repeat === "all" ? t("player.repeatAll") : t("player.repeat")
            }
            onClick={cycleRepeat}
          >
            <RepeatIcon mode={repeat} />
          </button>
        </div>

        <div className="pb-progress-row">
          <span className="pb-time">{formatDuration(positionMs)}</span>
          <div
            ref={barRef}
            className="progress-bar"
            role="slider"
            aria-valuemin={0}
            aria-valuemax={durationMs}
            aria-valuenow={Math.round(positionMs)}
            aria-label={t("player.seek")}
            onPointerDown={(e) => {
              e.currentTarget.setPointerCapture(e.pointerId);
              seekFromEvent(e);
            }}
            onPointerMove={(e) => {
              if (e.buttons & 1) seekFromEvent(e);
            }}
          >
            <div className="progress-fill" style={{ width: `${progress * 100}%` }}>
              <span className="progress-knob" />
            </div>
          </div>
          <span className="pb-time">{formatRemaining(Math.max(0, durationMs - positionMs))}</span>
        </div>
      </div>

      {/* Right cluster */}
      <div className="pb-right">
        {playback && playback.speed !== 1 && (
          <button
            className="chip speed-chip"
            title={t("settings.speed")}
            onClick={() => void api.setSpeed(playback.speed === 1 ? 1.25 : 1)}
          >
            {playback.speed}×
          </button>
        )}

        <div className="pb-popover-wrap" ref={timerRef}>
          <button
            className={`icon-btn ${sleepActive ? "active" : ""}`}
            title={t("player.sleepTimer")}
            onClick={() => setTimerOpen((o) => !o)}
          >
            <Timer size={17} />
            {sleepActive && <span className="badge-dot" />}
          </button>
          {timerOpen && (
            <div className="popover sleep-popover">
              <button
                className={`menu-item ${sleep.kind === "off" ? "selected" : ""}`}
                onClick={() => {
                  void api.setSleepTimer("off");
                  setTimerOpen(false);
                }}
              >
                {t("player.sleepOff")}
              </button>
              {timerOptions.map((opt) => (
                <button
                  key={opt.key}
                  className={`menu-item ${sleepKey === opt.key ? "selected" : ""}`}
                  onClick={() => {
                    void api.setSleepTimer(opt.kind, opt.minutes ?? null);
                    setTimerOpen(false);
                  }}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          )}
        </div>

        <div className="pb-popover-wrap">
          <button
            className={`icon-btn ${volumeOpen ? "active" : ""}`}
            title={t("player.volume")}
            onClick={() => setVolumeOpen((o) => !o)}
          >
            <VolumeIcon size={18} />
          </button>
          {volumeOpen && (
            <div className="popover volume-popover">
              <input
                type="range"
                min={0}
                max={1}
                step={0.01}
                value={volume}
                onChange={(e) => void api.setVolume(Number(e.target.value))}
                aria-label={t("player.volume")}
              />
              <span className="volume-value">{Math.round(volume * 100)}%</span>
            </div>
          )}
        </div>

        <button
          className={`icon-btn ${queueOpen ? "active" : ""}`}
          title={t("player.queue")}
          onClick={() => setQueueOpen(true)}
        >
          <ListMusic size={18} />
        </button>
        <button
          className="icon-btn pb-mini-btn"
          title={t("player.miniPlayer")}
          onClick={() => {
            api.setMiniPlayer(true).catch((e) => pushToast(String(e), "error"));
          }}
        >
          <Maximize2 size={17} />
        </button>
      </div>
    </footer>
  );
}
