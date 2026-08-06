import { useEffect } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { AppProvider, useApp, useGlobalShortcuts } from "./store";
import { Sidebar } from "./components/Sidebar";
import { TopBar } from "./components/TopBar";
import { PlayerBar } from "./components/PlayerBar";
import { QueueDrawer } from "./components/QueueDrawer";
import { NowPlayingDrawer } from "./components/NowPlayingDrawer";
import { ContextMenu } from "./components/ContextMenu";
import { Toasts } from "./components/Toasts";
import { ResumeBanner } from "./components/ResumeBanner";
import { MiniPlayer } from "./components/MiniPlayer";
import { ViewRouter } from "./components/ViewRouter";
import * as api from "./api";

function Shell() {
  useGlobalShortcuts();
  const { mini } = useApp();

  // Drag & drop of audio files onto the window imports them into the library.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop") {
          void api.importPaths(event.payload.paths);
        }
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => undefined);
    return () => unlisten?.();
  }, []);

  if (mini) {
    return <MiniPlayer />;
  }

  return (
    <div className="app-shell">
      <Sidebar />
      <div className="main-column">
        <TopBar />
        <main className="scroll-area">
          <ViewRouter />
        </main>
        <PlayerBar />
      </div>
      <QueueDrawer />
      <NowPlayingDrawer />
      <ContextMenu />
      <Toasts />
      <ResumeBanner />
    </div>
  );
}

export default function App() {
  return (
    <AppProvider>
      <Shell />
    </AppProvider>
  );
}
