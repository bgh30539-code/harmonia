import { Disc3, Heart, Library, Search, Settings } from "lucide-react";
import { useApp, type View } from "../store";

/**
 * Material-style bottom navigation for phones. Rendered on every view but
 * only shown below 720px; the full navigation lives in the sidebar, which
 * stays reachable through the top-bar menu button.
 */
export function BottomNav() {
  const { t, view, navigate } = useApp();

  const items: {
    key: string;
    label: string;
    icon: React.ReactNode;
    view: View;
    onTap?: () => void;
  }[] = [
    { key: "library", label: t("nav.library"), icon: <Library size={20} />, view: { name: "library" } },
    { key: "albums", label: t("nav.albums"), icon: <Disc3 size={20} />, view: { name: "albums" } },
    {
      key: "search",
      label: t("search.title"),
      icon: <Search size={20} />,
      view: { name: "search", query: "" },
      onTap: () => window.dispatchEvent(new CustomEvent("harmonia:focus-search")),
    },
    { key: "favorites", label: t("nav.favorites"), icon: <Heart size={20} />, view: { name: "favorites" } },
    { key: "settings", label: t("nav.settings"), icon: <Settings size={20} />, view: { name: "settings" } },
  ];

  const isActive = (v: View) => v.name === view.name;

  return (
    <nav className="bottom-nav" aria-label={t("app.name")}>
      {items.map((item) => (
        <button
          key={item.key}
          className={`bottom-nav-item ${isActive(item.view) ? "active" : ""}`}
          onClick={() => {
            navigate(item.view);
            item.onTap?.();
          }}
        >
          {item.icon}
          <span>{item.label}</span>
        </button>
      ))}
    </nav>
  );
}
