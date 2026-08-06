import { useEffect, useState } from "react";
import { Play } from "lucide-react";
import { useApp } from "../store";
import * as api from "../api";
import type { Track } from "../types";
import { TrackTable } from "../components/TrackTable";

export function MostPlayedView() {
  const { t, pushToast } = useApp();
  const [tracks, setTracks] = useState<Track[]>([]);

  useEffect(() => {
    api.getMostPlayed(200).then(setTracks).catch(() => undefined);
  }, []);

  return (
    <div className="view">
      <div className="view-header">
        <h1>{t("nav.mostPlayed")}</h1>
        <div className="view-actions">
          <button
            className="btn btn-primary"
            disabled={tracks.length === 0}
            onClick={() =>
              void api.playContext(tracks.map((tr) => tr.id), 0).catch((e) => pushToast(String(e), "error"))
            }
          >
            <Play size={15} fill="currentColor" className="play-glyph" />
            <span>{t("player.play")}</span>
          </button>
        </div>
      </div>
      <TrackTable tracks={tracks} emptyMessage={t("emptyState.mostPlayed")} />
    </div>
  );
}
