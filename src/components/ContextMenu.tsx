import { useEffect, useRef, useState } from "react";
import { ChevronRight } from "lucide-react";
import { useApp, type MenuAction } from "../store";

function MenuList({
  items,
  onClose,
  depth,
  onOpenSub,
  activeSub,
}: {
  items: MenuAction[];
  onClose: () => void;
  depth: number;
  onOpenSub: (id: string | null, top: number) => void;
  activeSub: string | null;
}) {
  const { t } = useApp();
  return (
    <ul className={`context-menu ${depth > 0 ? "submenu" : ""}`} role="menu">
      {items.map((item, i) =>
        item.separator ? (
          <li key={`sep-${i}`} className="menu-separator" />
        ) : (
          <li
            key={item.id}
            role="menuitem"
            className={`menu-item ${item.danger ? "danger" : ""} ${item.disabled ? "disabled" : ""} ${
              activeSub === item.id ? "open" : ""
            }`}
            onMouseEnter={(e) =>
              onOpenSub(item.children ? item.id : null, e.currentTarget.offsetTop)
            }
            onClick={() => {
              if (item.children) return;
              onClose();
              item.onSelect?.();
            }}
          >
            <span className="menu-label">{item.label}</span>
            {item.children && <ChevronRight size={14} className="menu-chevron" />}
          </li>
        ),
      )}
      {depth > 0 && (
        <li className="menu-item menu-back" onClick={onClose}>
          <span className="menu-label">{t("close")}</span>
        </li>
      )}
    </ul>
  );
}

export function ContextMenu() {
  const { contextMenu, closeContextMenu } = useApp();
  const [activeSub, setActiveSub] = useState<{ id: string; top: number } | null>(null);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!contextMenu) return;
    const close = (e: MouseEvent | KeyboardEvent) => {
      if (e instanceof KeyboardEvent && e.key === "Escape") {
        closeContextMenu();
        return;
      }
      if (ref.current && !ref.current.contains(e.target as Node)) {
        closeContextMenu();
      }
    };
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", close);
    window.addEventListener("blur", closeContextMenu);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", close);
      window.removeEventListener("blur", closeContextMenu);
    };
  }, [contextMenu, closeContextMenu]);

  useEffect(() => {
    setActiveSub(null);
  }, [contextMenu]);

  if (!contextMenu) return null;

  const parent = activeSub
    ? contextMenu.items.find((i) => i.id === activeSub.id && i.children)
    : null;
  const subItems = parent?.children ?? [];

  // Clamp the root menu to the viewport (estimate 34px per item).
  const rootH = contextMenu.items.length * 34 + 16;
  const x = Math.max(4, Math.min(contextMenu.x, window.innerWidth - 224 - 4));
  const y = Math.max(4, Math.min(contextMenu.y, window.innerHeight - rootH - 4));

  // Position the submenu next to the hovered item; flip left when it would
  // overflow the right edge, and keep it vertically inside the viewport.
  const subH = subItems.length * 34 + 16;
  const subLeft = x + 224 > window.innerWidth - 224 ? -224 : 224;
  const subTop = Math.max(-y + 4, Math.min(activeSub?.top ?? 0, window.innerHeight - y - subH - 4));

  return (
    <div ref={ref} className="context-menu-root" style={{ left: x, top: y }}>
      <MenuList
        items={contextMenu.items}
        onClose={closeContextMenu}
        depth={0}
        onOpenSub={(id, top) => setActiveSub(id ? { id, top } : null)}
        activeSub={activeSub?.id ?? null}
      />
      {subItems.length > 0 && (
        <div className="context-menu-root submenu-root" style={{ left: subLeft, top: subTop }}>
          <MenuList
            items={subItems}
            onClose={closeContextMenu}
            depth={1}
            onOpenSub={() => undefined}
            activeSub={null}
          />
        </div>
      )}
    </div>
  );
}
