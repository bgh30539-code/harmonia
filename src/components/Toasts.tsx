import { AlertCircle, CheckCircle2, Info, X } from "lucide-react";
import { useApp } from "../store";

export function Toasts() {
  const { toasts, dismissToast } = useApp();
  return (
    <div className="toasts" aria-live="polite">
      {toasts.map((toast) => (
        <div key={toast.id} className={`toast toast-${toast.kind}`}>
          {toast.kind === "error" ? (
            <AlertCircle size={16} />
          ) : toast.kind === "success" ? (
            <CheckCircle2 size={16} />
          ) : (
            <Info size={16} />
          )}
          <span className="toast-message">{toast.message}</span>
          <button className="toast-close" onClick={() => dismissToast(toast.id)}>
            <X size={13} />
          </button>
        </div>
      ))}
    </div>
  );
}
