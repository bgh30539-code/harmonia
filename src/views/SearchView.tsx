import { useEffect, useMemo, useState } from "react";
import { Filter, Search } from "lucide-react";
import { useApp } from "../store";
import * as api from "../api";
import type { SearchFilters, Track } from "../types";
import { TrackTable } from "../components/TrackTable";

const GENRES = [
  "Rock", "Pop", "Jazz", "Classical", "Electronic", "Hip-Hop", "Rap", "R&B", "Blues",
  "Country", "Metal", "Punk", "Folk", "Reggae", "Soul", "Funk", "Ambient", "House",
  "Techno", "Trance", "Drum & Bass", "Dubstep", "Latin", "World", "Soundtrack", "Gospel",
];

export function SearchView({ query }: { query: string }) {
  const { t, pushToast } = useApp();
  const [tracks, setTracks] = useState<Track[]>([]);
  const [showFilters, setShowFilters] = useState(false);
  const [filters, setFilters] = useState<SearchFilters>({});

  useEffect(() => {
    setFilters({});
    setShowFilters(false);
  }, [query]);

  useEffect(() => {
    const q = query.trim();
    if (!q) {
      setTracks([]);
      return;
    }
    let cancelled = false;
    api
      .searchLibrary(q, filters)
      .then((results) => {
        if (!cancelled) setTracks(results);
      })
      .catch((e) => {
        if (!cancelled) pushToast(String(e), "error");
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, filters]);

  const set = (patch: SearchFilters) => setFilters((f) => ({ ...f, ...patch }));

  const filterControls = useMemo(
    () => (
      <div className="filter-panel">
        <label>
          <span>{t("genres")}</span>
          <select
            className="select"
            value={filters.genre ?? ""}
            onChange={(e) => set({ genre: e.target.value || null })}
          >
            <option value="">—</option>
            {GENRES.map((g) => (
              <option key={g} value={g}>
                {g}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>{t("codec")}</span>
          <select
            className="select"
            value={filters.format ?? ""}
            onChange={(e) => set({ format: e.target.value || null })}
          >
            <option value="">—</option>
            {["MP3", "FLAC", "OGG", "AAC", "M4A", "WAV"].map((f) => (
              <option key={f} value={f}>
                {f}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>{t("table.year")} ≥</span>
          <input
            type="number"
            className="input"
            placeholder="1990"
            value={filters.yearMin ?? ""}
            onChange={(e) => set({ yearMin: e.target.value ? Number(e.target.value) : null })}
          />
        </label>
        <label>
          <span>{t("table.year")} ≤</span>
          <input
            type="number"
            className="input"
            placeholder="2026"
            value={filters.yearMax ?? ""}
            onChange={(e) => set({ yearMax: e.target.value ? Number(e.target.value) : null })}
          />
        </label>
        <label>
          <span>{t("bitrate")} ≥</span>
          <input
            type="number"
            className="input"
            placeholder="128"
            value={filters.bitrateMin ?? ""}
            onChange={(e) => set({ bitrateMin: e.target.value ? Number(e.target.value) : null })}
          />
        </label>
        <label>
          <span>{t("table.duration")} ≤ (min)</span>
          <input
            type="number"
            className="input"
            placeholder="10"
            value={filters.durationMaxMs ? Math.round(filters.durationMaxMs / 60000) : ""}
            onChange={(e) =>
              set({ durationMaxMs: e.target.value ? Number(e.target.value) * 60000 : null })
            }
          />
        </label>
        <button className="btn btn-ghost clear-filters" onClick={() => setFilters({})}>
          {t("close")}
        </button>
      </div>
    ),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [filters],
  );

  return (
    <div className="view">
      <div className="view-header">
        <h1>
          {t("search.title")}
          {query.trim() && <span className="view-count">“{query}”</span>}
        </h1>
        <button
          className={`btn btn-ghost ${showFilters ? "active" : ""}`}
          onClick={() => setShowFilters((s) => !s)}
        >
          <Filter size={15} />
          <span>{t("search.filters")}</span>
        </button>
      </div>

      {showFilters && filterControls}

      {!query.trim() ? (
        <div className="empty-state hero">
          <Search size={48} strokeWidth={1.2} />
          <p>{t("search.placeholder")}</p>
        </div>
      ) : (
        <TrackTable
          tracks={tracks}
          emptyMessage={t("search.empty", { query })}
        />
      )}
    </div>
  );
}
