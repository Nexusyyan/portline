import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown, Search } from "lucide-react";

export interface SelectOption {
  value: string;
  label: string;
  meta?: string;
  icon?: ReactNode;
}

export interface SelectGroup {
  label: string;
  options: SelectOption[];
}

interface CustomSelectProps {
  value: string;
  groups: SelectGroup[];
  onChange: (value: string) => void;
  placeholder?: string;
  searchPlaceholder?: string;
  emptyLabel?: string;
  className?: string;
  size?: "sm" | "md";
  disabled?: boolean;
}

interface PopoverRect {
  top: number;
  left: number;
  width: number;
  maxHeight: number;
  openUpward: boolean;
}

export function CustomSelect({
  value,
  groups,
  onChange,
  placeholder = "",
  searchPlaceholder = "Filter...",
  emptyLabel = "No matches",
  className = "",
  size = "md",
  disabled = false,
}: CustomSelectProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [rect, setRect] = useState<PopoverRect | null>(null);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const menuRef = useRef<HTMLDivElement | null>(null);
  const searchRef = useRef<HTMLInputElement | null>(null);

  const flatOptions = useMemo(
    () => groups.flatMap((g) => g.options),
    [groups]
  );

  const selectedOption = useMemo(
    () => flatOptions.find((o) => o.value === value) ?? null,
    [flatOptions, value]
  );

  const showSearch = flatOptions.length > 10;

  const filteredGroups = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return groups.filter((g) => g.options.length > 0);
    return groups
      .map((g) => ({
        ...g,
        options: g.options.filter(
          (o) =>
            o.label.toLowerCase().includes(q) ||
            (o.meta && o.meta.toLowerCase().includes(q)) ||
            o.value.toLowerCase().includes(q)
        ),
      }))
      .filter((g) => g.options.length > 0);
  }, [groups, query]);

  const computePosition = () => {
    const el = triggerRef.current;
    if (!el) return;
    const box = el.getBoundingClientRect();
    const spaceBelow = window.innerHeight - box.bottom - 12;
    const spaceAbove = box.top - 12;
    const desired = 280;
    const openUpward = spaceBelow < 190 && spaceAbove > spaceBelow;
    const avail = openUpward ? spaceAbove : spaceBelow;
    const maxHeight = Math.max(140, Math.min(desired, avail));
    const width = Math.max(box.width, 220);
    const left = Math.min(
      Math.max(8, box.left),
      Math.max(8, window.innerWidth - width - 8)
    );
    const top = openUpward ? box.top - 6 : box.bottom + 6;
    setRect({ top, left, width, maxHeight, openUpward });
  };

  useEffect(() => {
    if (!open) {
      setQuery("");
      return;
    }
    computePosition();
    const timer = window.setTimeout(() => {
      if (showSearch) searchRef.current?.focus();
    }, 10);

    const onPointerDown = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        triggerRef.current?.contains(target) ||
        menuRef.current?.contains(target)
      ) {
        return;
      }
      setOpen(false);
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setOpen(false);
        triggerRef.current?.focus();
      }
    };
    const onViewportChange = () => computePosition();

    window.addEventListener("mousedown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("resize", onViewportChange);
    window.addEventListener("scroll", onViewportChange, true);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("mousedown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", onViewportChange);
      window.removeEventListener("scroll", onViewportChange, true);
    };
  }, [open, showSearch]);

  const heightCls = size === "sm" ? "h-7 px-2.5 text-[12px]" : "h-8 px-2.5 text-[12px]";

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => {
          if (!disabled) setOpen((v) => !v);
        }}
        className={`ui-select inline-flex items-center justify-between gap-2 text-left cursor-pointer ${heightCls} ${
          open ? "border-[var(--border-focus)] bg-[var(--bg-surface)]" : ""
        } ${disabled ? "opacity-45 cursor-not-allowed" : ""} ${className}`}
      >
        <span className="flex min-w-0 flex-1 items-center justify-between gap-2">
          <span className="flex min-w-0 items-center gap-1.5 truncate text-[var(--text-primary)]">
            {selectedOption?.icon}
            <span className="truncate">
              {selectedOption ? selectedOption.label : placeholder}
            </span>
          </span>
          {selectedOption?.meta && (
            <span className="shrink-0 font-mono text-[11px] num-tabular text-[var(--text-tertiary)]">
              {selectedOption.meta}
            </span>
          )}
        </span>
        <ChevronDown
          className={`h-3.5 w-3.5 shrink-0 text-[var(--text-tertiary)] transition-transform ${
            open ? "rotate-180" : ""
          }`}
          strokeWidth={1.5}
        />
      </button>

      {open &&
        rect &&
        createPortal(
          <div
            ref={menuRef}
            role="listbox"
            style={{
              position: "fixed",
              left: `${rect.left}px`,
              width: `${rect.width}px`,
              top: rect.openUpward ? undefined : `${rect.top}px`,
              bottom: rect.openUpward
                ? `${window.innerHeight - rect.top}px`
                : undefined,
              zIndex: 80,
            }}
            className="ui-popover flex flex-col overflow-hidden"
          >
            {showSearch && (
              <div className="border-b border-[var(--border-hairline)] p-1.5">
                <div className="relative">
                  <Search
                    className="pointer-events-none absolute top-1/2 left-2.5 h-3.5 w-3.5 -translate-y-1/2 text-[var(--text-tertiary)]"
                    strokeWidth={1.5}
                  />
                  <input
                    ref={searchRef}
                    type="text"
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                    placeholder={searchPlaceholder}
                    className="ui-input h-7 w-full pr-2.5 pl-8 text-[11.5px]"
                  />
                </div>
              </div>
            )}

            <div
              style={{ maxHeight: `${rect.maxHeight}px` }}
              className="overflow-auto p-1"
            >
              {filteredGroups.length === 0 ? (
                <div className="px-3 py-4 text-center text-[11.5px] text-[var(--text-tertiary)]">
                  {emptyLabel}
                </div>
              ) : (
                filteredGroups.map((group, gIdx) => (
                  <div
                    key={group.label}
                    className={
                      gIdx > 0
                        ? "mt-1 border-t border-[var(--border-hairline)] pt-1"
                        : ""
                    }
                  >
                    <div className="px-2.5 py-1 text-[10.5px] font-medium text-[var(--text-tertiary)] select-none">
                      {group.label}
                    </div>
                    <div className="space-y-0.5">
                      {group.options.map((opt) => {
                        const active = opt.value === value;
                        return (
                          <button
                            key={opt.value}
                            type="button"
                            role="option"
                            aria-selected={active}
                            onClick={() => {
                              onChange(opt.value);
                              setOpen(false);
                            }}
                            className={`flex w-full items-center justify-between gap-2 rounded-[7px] px-2.5 py-1.5 text-left text-[12px] transition-colors cursor-pointer ${
                              active
                                ? "bg-[var(--bg-selected)] font-medium text-[var(--text-primary)]"
                                : "text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"
                            }`}
                          >
                            <span className="flex min-w-0 items-center gap-1.5 truncate">
                              {opt.icon}
                              <span className="truncate">{opt.label}</span>
                            </span>
                            <span className="flex shrink-0 items-center gap-1.5">
                              {opt.meta && (
                                <span className="font-mono text-[11px] num-tabular text-[var(--text-secondary)]">
                                  {opt.meta}
                                </span>
                              )}
                              {active && (
                                <Check
                                  className="h-3.5 w-3.5 text-[var(--text-primary)]"
                                  strokeWidth={1.7}
                                />
                              )}
                            </span>
                          </button>
                        );
                      })}
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>,
          document.body
        )}
    </>
  );
}
