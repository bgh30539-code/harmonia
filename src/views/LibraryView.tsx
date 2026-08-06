import { useCallback, useEffect, useRef, useState } from "react";
import { FolderPlus, Library, RefreshCw } from "lucide-react";
import { useApp } from "../store";
import * as api from "../api";
import type { Folder, LibraryStats, SortField, Track } from "../types";
import { TrackTable } from "../components/TrackTable";
import { formatTotalDuration } from "../utils";

const PAGE = 500;

export function LibraryView() {
  const { t, libraryVersion, pushToast } = useApp();
  const [tracks, setTracks] = useState<Track[]>([]);
  const [stats, setStats] = useState<LibraryStats | null>(null);
  const [folders, setFolders] = useState<Folder[]>([]);
  const [folder, setFolder] = useState<string | null>(null);
  const [sort, setSort] = useState<SortField>("title");
  const [desc, setDesc] = useState(false);
  const [hasMore, setHasMore] = useState(true);
  const offsetRef = useRef(0);
  const folderRef = useRef(folder);
  const sortRef = useRef(sort);
  const descRef = useRef(desc);
  folderRef.current = folder;
  sortRef.current = sort;
  descRef.current = desc;

  const loadPage = useCallback((append: boolean) => {
    const off = append ? offsetRef.current : 0;
    api
      .getTracks(off, PAGE, sortRef.current, descRef.current, folderRef.current ?? null)
      .then((page) => {
        offsetRef.current = off + page.length;
        setTracks((prev) => (append ? [...prev, ...page] : page));
        setHasMore(page.length === PAGE);
      })
      .catch(() => undefined);
  }, []);

  // Reload from scratch whenever the library changes or the filter/sort changes.
  useEffect(() => {
    offsetRef.current = 0;
    setHasMore(true);
    loadPage(false);
  }, [libraryVersion, folder, sort, desc, loadPage]);

  useEffect(() => {
    api.getStats().then(setStats).catch(() => undefined);
    api.listFolders().then(setFolders).catch(() => undefined);
  }, [libraryVersion]);

  const loadMore = () => {
    if (hasMore) loadPage(true);
  };

  const empty = tracks.length === 0 && (stats?.tracks ?? 0) === 0;

  return (
    <div className="view">
      <div className="view-header">
        <div>
          <h1>{t("nav.library")}</h1>
          {stats && (
            <div className="stats-row">
              <span>
                {stats.tracks} {t("library.tracks")}
              </span>
              <span>
                {stats.albums} {t("library.albums")}
              </span>
              <span>
                {stats.artists} {t("library.artists")}
              </span>
              <span>{formatTotalDuration(stats.totalDurationMs)}</span>
            </div>
          )}
        </div>
        <div className="view-actions">
          {folders.length > 1 && (
            <select
              className="select folder-select"
              value={folder ?? ""}
              onChange={(e) => setFolder(e.target.value || null)}
              aria-label={t("library.folders")}
            >
              <option value="">{t("library.folders")}</option>
              {folders.map((f) => (
                <option key={f.path} value={f.path}>
                  {f.path}
                </option>
              ))}
            </select>
          )}
          <button
            className="btn btn-ghost"
            title={t("library.rescan")}
            onClick={() => void api.scanLibrary(false).catch((e) => pushToast(String(e), "error"))}
          >
            <RefreshCw size={15} />
            <span>{t("library.rescan")}</span>
          </button>
          <button
            className="btn btn-primary"
            onClick={() => {
              api.addFolder().then((p) => {
                if (p) pushToast(t("library.folderAdded"), "success");
              });
            }}
          >
            <FolderPlus size={15} />
            <span>{t("library.addFolder")}</span>
          </button>
        </div>
      </div>

      {empty ? (
        <div className="empty-state hero">
          <Library size={56} strokeWidth={1.1} />
          <h2>{t("library.empty.title")}</h2>
          <p>{t("library.empty.body")}</p>
          <button
            className="btn btn-primary"
            onClick={() => {
              api.addFolder().then((p) => {
                if (p) pushToast(t("library.folderAdded"), "success");
              });
            }}
          >
            <FolderPlus size={15} />
            <span>{t("library.addFolder")}</span>
          </button>
        </div>
      ) : (
        <TrackTable
          tracks={tracks}
          loadMore={loadMore}
          showAlbum
          showArtist
          sort={sort}
          desc={desc}
          onSortChange={(s, d) => {
            setSort(s);
            setDesc(d);
          }}
        />
      )}
    </div>
  );
}
