import { useState } from "react";
import { GripVertical, Heart, Music } from "lucide-react";
import { useApp } from "../store";
import * as api from "../api";
import type { Playlist, SortField, Track } from "../types";
import { formatDuration } from "../utils";
import { Art } from "./Art";
import { useVirtual } from "../hooks/useVirtual";

const ROW_H = 48;

interface TrackTableProps {
  tracks: Track[];
  loadMore?: () => void;
  showAlbum?: boolean;
  showArtist?: boolean;
  showThumb?: boolean;
  playlistId?: number;
  onReorder?: (orderedIds: number[]) => void;
  onRemoveFromPlaylist?: (trackId: number) => void;
  sort?: SortField;
  desc?: boolean;
  onSortChange?: (sort: SortField, desc: boolean) => void;
  emptyMessage?: string;
}

export function TrackTable({
  tracks,
  loadMore,
  showAlbum = true,
  showArtist = true,
  showThumb = true,
  playlistId,
  onReorder,
  onRemoveFromPlaylist,
  sort,
  desc,
  onSortChange,
  emptyMessage,
}: TrackTableProps) {
  const { t, playingId, openContextMenu, pushToast, navigate } = useApp();
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [dropIndex, setDropIndex] = useState<number | null>(null);
  const list = useVirtual(tracks.length, ROW_H, loadMore);

  const sorters: { key: SortField; label: string; col: "title" | "artist" | "album" | "duration" | "year" | "playCount" | "dateAdded" }[] = [
    { key: "title", label: t("table.title"), col: "title" },
    { key: "artist", label: t("table.artist"), col: "artist" },
    { key: "album", label: t("table.album"), col: "album" },
    { key: "year", label: t("table.year"), col: "year" },
    { key: "playCount", label: t("table.plays"), col: "playCount" },
    { key: "duration", label: t("table.duration"), col: "duration" },
  ];

  const sortTracks = (a: Track, b: Track): number => {
    if (!sort) return 0;
    const dir = desc ? -1 : 1;
    switch (sort) {
      case "title":
        return a.title.localeCompare(b.title) * dir;
      case "artist":
        return a.artist.localeCompare(b.artist) * dir;
      case "album":
        return a.album.localeCompare(b.album) * dir;
      case "year":
        return ((a.year ?? 0) - (b.year ?? 0)) * dir;
      case "duration":
        return (a.durationMs - b.durationMs) * dir;
      case "playCount":
        return (a.playCount - b.playCount) * dir;
      default:
        return 0;
    }
  };

  const ordered = sort ? [...tracks].sort(sortTracks) : tracks;

  const toggleFavorite = (track: Track) => {
    const next = !track.favorite;
    api.setFavorite(track.id, next).then(() => {
      track.favorite = next;
      pushToast(next ? "♥" : "♡", "info");
    });
  };

  const openMenu = async (e: React.MouseEvent, track: Track) => {
    e.preventDefault();
    const playlists = await api.listPlaylists().catch(() => [] as Playlist[]);
    const items = [
      {
        id: "play",
        label: t("context.play"),
        onSelect: () => void api.playTrack(track.id),
      },
      {
        id: "queue",
        label: t("context.addToQueue"),
        onSelect: () => void api.addToQueue([track.id]),
      },
      {
        id: "playlist",
        label: t("context.addToPlaylist"),
        children: [
          ...playlists.map((p) => ({
            id: `pl-${p.id}`,
            label: p.name,
            onSelect: () => {
              api.addTracksToPlaylist(p.id, [track.id]).then(() => {
                pushToast(`→ ${p.name}`, "success");
              });
            },
          })),
        ],
      },
      {
        id: "fav",
        label: track.favorite ? t("context.unfavorite") : t("context.favorite"),
        onSelect: () => toggleFavorite(track),
      },
      track.album
        ? {
            id: "album",
            label: t("context.viewAlbum"),
            onSelect: () => navigate({ name: "album", title: track.album, artist: track.artist }),
          }
        : null,
      track.artist
        ? {
            id: "artist",
            label: t("context.viewArtist"),
            onSelect: () => navigate({ name: "artist", artist: track.artist }),
          }
        : null,
      ...(playlistId && onRemoveFromPlaylist
        ? [
            {
              id: "remove",
              label: t("context.removeFromPlaylist"),
              danger: true,
              onSelect: () => onRemoveFromPlaylist(track.id),
            },
          ]
        : []),
    ].filter(Boolean) as never[];

    openContextMenu(e.clientX, e.clientY, items as never);
  };

  const handleDrop = (to: number) => {
    setDragIndex(null);
    setDropIndex(null);
    if (dragIndex === null || dragIndex === to || !onReorder) return;
    const reordered = [...ordered];
    const [moved] = reordered.splice(dragIndex, 1);
    reordered.splice(to, 0, moved);
    onReorder(reordered.map((tr) => tr.id));
  };

  if (tracks.length === 0) {
    return (
      <div className="empty-state">
        <Music size={40} strokeWidth={1.2} />
        <p>{emptyMessage ?? t("emptyState.library")}</p>
      </div>
    );
  }

  return (
    <div className="track-table-wrap">
      <div className="track-table-header">
        <span className="col-index">#</span>
        <span className="col-grip" />
        {showThumb && <span className="col-thumb" />}
        {sorters.slice(0, showArtist && showAlbum ? 3 : 2).map((s) => (
          <button
            key={s.key}
            className={`col-sortable ${sort === s.key ? "sorted" : ""}`}
            onClick={() =>
              onSortChange?.(s.key, sort === s.key ? !desc : false)
            }
          >
            {s.label}
            {sort === s.key && <span className="sort-arrow">{desc ? "↓" : "↑"}</span>}
          </button>
        ))}
        {showArtist && !showAlbum && <span className="col-artist-only">{t("table.artist")}</span>}
        <span className="col-duration">{t("table.duration")}</span>
        <span className="col-fav" />
      </div>
      <div
        className="track-table-body"
        ref={list.containerRef}
        onScroll={list.onScroll}
        onMouseLeave={() => setDropIndex(null)}
      >
        <div style={{ height: list.totalHeight, position: "relative" }}>
          {ordered.slice(list.visibleStart, list.visibleEnd).map((track, i) => {
            const index = list.visibleStart + i;
            const isCurrent = track.id === playingId;
            const isDragOver = dropIndex === index;
            return (
              <div
                key={track.id}
                className={`track-row ${isCurrent ? "current" : ""}`}
                style={{ transform: `translateY(${index * ROW_H}px)`, height: ROW_H }}
                draggable={!!onReorder}
                onDragStart={() => setDragIndex(index)}
                onDragOver={(e) => {
                  if (onReorder) {
                    e.preventDefault();
                    setDropIndex(index);
                  }
                }}
                onDrop={() => handleDrop(index)}
                onDoubleClick={() => void api.playTrack(track.id)}
                onContextMenu={(e) => void openMenu(e, track)}
              >
                {onReorder && (
                  <span className="col-grip">
                    <GripVertical size={14} />
                  </span>
                )}
                {showThumb ? (
                  <span className="col-thumb">
                    <Art hash={track.artHash} alt={track.title} className="row-art" />
                  </span>
                ) : (
                  <span className="col-index">{isCurrent ? "▶" : index + 1}</span>
                )}
                <span className="col-title" title={track.path}>
                  <span className="track-title">{track.title || t("library.unknown")}</span>
                  {!showArtist && <span className="track-subtitle">{track.artist}</span>}
                </span>
                {showArtist && (
                  <span className="col-artist" onClick={() => navigate({ name: "artist", artist: track.artist })}>
                    {track.artist || t("library.unknown")}
                  </span>
                )}
                {showAlbum && (
                  <span className="col-album" onClick={() => navigate({ name: "album", title: track.album, artist: track.artist })}>
                    {track.album || t("library.unknown")}
                  </span>
                )}
                <span className="col-duration">{formatDuration(track.durationMs)}</span>
                <button
                  className={`col-fav ${track.favorite ? "faved" : ""}`}
                  onClick={() => toggleFavorite(track)}
                  title={track.favorite ? t("context.unfavorite") : t("context.favorite")}
                >
                  <Heart size={15} fill={track.favorite ? "currentColor" : "none"} />
                </button>
                {isDragOver && <span className="drop-indicator" />}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
