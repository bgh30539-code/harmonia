import { useEffect, useRef, useState } from "react";
import { FolderPlus, Loader2, Menu, Search, RefreshCw } from "lucide-react";
import { useApp } from "../store";
import * as api from "../api";
import type { ScanProgress } from "../types";

export function TopBar({ onMenu }: { onMenu?: () => void }) {
  const { t, navigate, pushToast } = useApp();
  const [query, setQuery] = useState("");
  const [scan, setScan] = useState<ScanProgress | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const focus = () => inputRef.current?.focus();
    window.addEventListener("harmonia:focus-search", focus);
    return () => window.removeEventListener("harmonia:focus-search", focus);
  }, []);

  useEffect(() => {
    const subs = [
      api.onEvent("scan://progress", setScan),
      api.onEvent("scan://done", () => setScan(null)),
    ];
    return () => {
      subs.forEach((p) => p.then((u) => u()).catch(() => undefined));
    };
  }, []);

  const submit = (value: string) => {
    const q = value.trim();
    if (q) navigate({ name: "search", query: q });
    else if (queryRef.current) navigate({ name: "library" });
  };

  // Keep the local input in sync with the view.
  const queryRef = useRef(query);
  queryRef.current = query;

  return (
    <header className="topbar">
      <button className="icon-btn topbar-menu" title={t("nav.menu")} onClick={onMenu}>
        <Menu size={18} />
      </button>
      <div className="search-box">
        <Search size={16} className="search-icon" />
        <input
          ref={inputRef}
          value={query}
          placeholder={t("search.placeholder")}
          onChange={(e) => {
            setQuery(e.target.value);
            submit(e.target.value);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") submit((e.target as HTMLInputElement).value);
            if (e.key === "Escape") {
              setQuery("");
              navigate({ name: "library" });
            }
          }}
        />
        {scan && (
          <span className="scan-indicator" title={scan.phase}>
            <Loader2 size={15} className="spin" />
            {scan.phase === "parse" && scan.total > 0
              ? `${Math.round((scan.current / scan.total) * 100)}%`
              : t("library.scanning")}
          </span>
        )}
      </div>
      <div className="topbar-actions">
        <button
          className="btn btn-ghost"
          title={t("library.rescan")}
          onClick={() => {
            api.scanLibrary(false).catch((e) => pushToast(String(e), "error"));
          }}
        >
          <RefreshCw size={16} />
        </button>
        <button
          className="btn btn-primary"
          onClick={() => {
            api
              .addFolder()
              .then((p) => {
                if (p) pushToast(t("library.folderAdded"), "success");
              })
              .catch((e) => pushToast(String(e), "error"));
          }}
        >
          <FolderPlus size={16} />
          <span>{t("library.addFolder")}</span>
        </button>
      </div>
    </header>
  );
}
