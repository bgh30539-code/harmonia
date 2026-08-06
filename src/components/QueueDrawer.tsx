import { Trash2, X } from "lucide-react";
import { useApp } from "../store";
import * as api from "../api";
import { formatDuration } from "../utils";
import { Art } from "./Art";

export function QueueDrawer() {
  const { playback, queueOpen, setQueueOpen, t } = useApp();
  const queue = playback?.queue ?? [];
  const currentIndex = playback?.queueIndex ?? -1;

  if (!queueOpen) return null;

  return (
    <aside className="drawer queue-drawer">
      <div className="drawer-header">
        <h3>{t("queue.title")}</h3>
        <div className="drawer-actions">
          {queue.length > 0 && (
            <button className="icon-btn" title={t("queue.clear")} onClick={() => void api.clearQueue()}>
              <Trash2 size={15} />
            </button>
          )}
          <button className="icon-btn" onClick={() => setQueueOpen(false)}>
            <X size={16} />
          </button>
        </div>
      </div>

      <div className="queue-list">
        {queue.length === 0 ? (
          <div className="empty-state compact">
            <p>{t("queue.empty")}</p>
          </div>
        ) : (
          queue.map((track, i) => {
            const isCurrent = i === currentIndex;
            return (
              <div
                key={`${track.id}-${i}`}
                className={`queue-row ${isCurrent ? "current" : ""}`}
                onClick={() => {
                  if (isCurrent) return;
                  void api.playContext(queue.map((tr) => tr.id), i);
                }}
              >
                <span className="queue-index">{isCurrent ? "▶" : i + 1}</span>
                <Art hash={track.artHash} alt={track.title} className="queue-art" />
                <div className="queue-meta">
                  <span className="queue-title">{track.title || t("library.unknown")}</span>
                  <span className="queue-sub">{track.artist || t("library.unknown")}</span>
                </div>
                <span className="queue-duration">{formatDuration(track.durationMs)}</span>
                <button
                  className="icon-btn queue-remove"
                  title={t("close")}
                  onClick={(e) => {
                    e.stopPropagation();
                    void api.removeQueueItem(i);
                  }}
                >
                  <X size={13} />
                </button>
              </div>
            );
          })
        )}
      </div>
    </aside>
  );
}
