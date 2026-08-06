import { useEffect, useState } from "react";
import { Play, X } from "lucide-react";
import { useApp } from "../store";
import * as api from "../api";
import type { ResumeInfo } from "../types";

export function ResumeBanner() {
  const { t, settings } = useApp();
  const [info, setInfo] = useState<ResumeInfo | null>(null);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    if (!settings?.resumeLastSession) return;
    api
      .resumeInfo()
      .then((r) => setInfo(r))
      .catch(() => undefined);
  }, [settings?.resumeLastSession]);

  if (!info || dismissed) return null;

  const resume = () => {
    setInfo(null);
    void api.continueSession();
  };

  return (
    <div className="resume-banner">
      <button className="resume-btn" onClick={resume}>
        <Play size={14} fill="currentColor" />
        <span>{t("player.resume")}</span>
      </button>
      <button className="icon-btn" title={t("close")} onClick={() => setDismissed(true)}>
        <X size={14} />
      </button>
    </div>
  );
}
