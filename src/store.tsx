import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import * as api from "./api";
import { translate } from "./i18n";
import { applyTheme } from "./utils";
import type { PlaybackSnapshot, Settings, Toast } from "./types";

export type View =
  | { name: "library" }
  | { name: "albums" }
  | { name: "artists" }
  | { name: "playlists" }
  | { name: "favorites" }
  | { name: "recent" }
  | { name: "mostPlayed" }
  | { name: "settings" }
  | { name: "search"; query: string }
  | { name: "album"; title: string; artist: string }
  | { name: "artist"; artist: string }
  | { name: "playlist"; id: number; title: string };

export interface MenuAction {
  id: string;
  label: string;
  icon?: React.ReactNode;
  danger?: boolean;
  separator?: boolean;
  disabled?: boolean;
  children?: MenuAction[];
  onSelect?: () => void;
}

interface ContextMenuState {
  x: number;
  y: number;
  items: MenuAction[];
}

interface AppContextValue {
  settings: Settings | null;
  playback: PlaybackSnapshot | null;
  positionMs: number;
  view: View;
  navigate: (view: View) => void;
  libraryVersion: number;
  toasts: Toast[];
  pushToast: (message: string, kind?: Toast["kind"]) => void;
  dismissToast: (id: number) => void;
  queueOpen: boolean;
  setQueueOpen: (open: boolean) => void;
  nowPlayingOpen: boolean;
  setNowPlayingOpen: (open: boolean) => void;
  mini: boolean;
  contextMenu: ContextMenuState | null;
  openContextMenu: (x: number, y: number, items: MenuAction[]) => void;
  closeContextMenu: () => void;
  t: (key: string, vars?: Record<string, string | number>) => string;
  playingId: number | null;
}

const AppContext = createContext<AppContextValue | null>(null);

let toastId = 0;

export function AppProvider({ children }: { children: React.ReactNode }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [playback, setPlayback] = useState<PlaybackSnapshot | null>(null);
  const [positionMs, setPositionMs] = useState(0);
  const [view, setView] = useState<View>({ name: "library" });
  const [libraryVersion, setLibraryVersion] = useState(0);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [queueOpen, setQueueOpen] = useState(false);
  const [nowPlayingOpen, setNowPlayingOpen] = useState(false);
  const [mini, setMini] = useState(false);
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const viewRef = useRef(view);
  viewRef.current = view;

  const pushToast = useCallback((message: string, kind: Toast["kind"] = "info") => {
    const id = ++toastId;
    setToasts((prev) => [...prev.slice(-3), { id, message, kind }]);
    window.setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 4200);
  }, []);

  const dismissToast = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const navigate = useCallback((next: View) => {
    setView(next);
    setQueueOpen(false);
    setNowPlayingOpen(false);
  }, []);

  const openContextMenu = useCallback((x: number, y: number, items: MenuAction[]) => {
    setContextMenu({ x, y, items });
  }, []);

  const closeContextMenu = useCallback(() => setContextMenu(null), []);

  // Load settings + initial playback state.
  useEffect(() => {
    api.getSettings().then(setSettings).catch(() => undefined);
    api
      .getPlayback()
      .then((snap) => {
        setPlayback(snap);
        setPositionMs(snap.positionMs);
      })
      .catch(() => undefined);
  }, []);

  // Subscribe to backend events.
  useEffect(() => {
    const subs = [
      api.onEvent("player://state", (snap) => {
        setPlayback(snap);
        setPositionMs(snap.positionMs);
      }),
      api.onEvent("player://position", (p) => setPositionMs(p.positionMs)),
      api.onEvent("scan://done", (stats) => {
        const s = settingsRef.current;
        const t = (key: string, vars?: Record<string, string | number>) =>
          translate(s?.language ?? "en", key, vars);
        pushToast(
          t("library.scanDone", {
            added: stats.added,
            updated: stats.updated,
            removed: stats.removed,
          }),
          "success",
        );
      }),
      api.onEvent("library://changed", () => setLibraryVersion((v) => v + 1)),
      api.onEvent("toast://show", (payload) => {
        pushToast(
          payload.message,
          payload.kind === "error" ? "error" : payload.kind === "success" ? "success" : "info",
        );
      }),
      api.onEvent("ui://mini", (enabled) => setMini(enabled)),
      api.onEvent("settings://changed", (s) => setSettings(s)),
    ];
    return () => {
      subs.forEach((p) => p.then((unsub) => unsub()).catch(() => undefined));
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Live-apply the resolved theme to the document so visual settings take
  // effect the moment they change (also covers unsaved drafts in the UI).
  useEffect(() => {
    return applyTheme(settings?.theme ?? "system", settings?.accent ?? "#7c5cff");
  }, [settings?.theme, settings?.accent]);

  const settingsRef = useRef(settings);
  settingsRef.current = settings;

  const lang = settings?.language ?? "en";
  const t = useCallback(
    (key: string, vars?: Record<string, string | number>) => translate(lang, key, vars),
    [lang],
  );

  const value: AppContextValue = {
    settings,
    playback,
    positionMs,
    view,
    navigate,
    libraryVersion,
    toasts,
    pushToast,
    dismissToast,
    queueOpen,
    setQueueOpen,
    nowPlayingOpen,
    setNowPlayingOpen,
    mini,
    contextMenu,
    openContextMenu,
    closeContextMenu,
    t,
    playingId: playback?.current?.id ?? null,
  };

  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}

export function useApp(): AppContextValue {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("useApp must be used inside AppProvider");
  return ctx;
}

/** Global keyboard shortcuts (in-app; media keys are handled by the backend). */
export function useGlobalShortcuts() {
  const { settings, playback } = useApp();
  const positionRef = useRef(0);
  const volumeRef = useRef(settings?.volume ?? 0.9);
  positionRef.current = playback?.positionMs ?? 0;
  volumeRef.current = settings?.volume ?? 0.9;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      const typing =
        target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;
      const ctrl = e.ctrlKey || e.metaKey;

      if (ctrl && e.key.toLowerCase() === "f") {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent("harmonia:focus-search"));
        return;
      }
      if (typing) return;

      switch (e.key) {
        case " ":
          e.preventDefault();
          void api.togglePlayback();
          break;
        case "ArrowRight":
          if (ctrl) void api.playNext();
          else void api.seek(positionRef.current + 5000);
          break;
        case "ArrowLeft":
          if (ctrl) void api.playPrevious();
          else void api.seek(positionRef.current - 5000);
          break;
        case "ArrowUp":
          e.preventDefault();
          void api.setVolume(Math.min(1, volumeRef.current + 0.05));
          break;
        case "ArrowDown":
          e.preventDefault();
          void api.setVolume(Math.max(0, volumeRef.current - 0.05));
          break;
        case "m":
          void api.setVolume(volumeRef.current > 0 ? 0 : 0.8);
          break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
