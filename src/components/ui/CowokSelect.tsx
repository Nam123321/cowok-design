// @story 04.1, 06.1
// @implements AC-06 (Semantic Variable Custom Dropdown with Viewport Portal across Warm/Dim/Night)
import React, { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "../../hooks/useTranslation";
import { BrandIcon } from "./BrandIcon";

export interface CowokSelectOption {
  value: string;
  label: string;
}

export interface CowokSelectProps {
  id?: string;
  testId?: string;
  value: string;
  options: CowokSelectOption[];
  onChange: (value: string) => void;
  ariaLabel?: string;
  placeholder?: string;
}

interface MenuCoords {
  top: number;
  left: number;
  width: number;
  openUpward: boolean;
}

const CHEVRON_SIZE = 14;
const CHECK_SIZE = 13;
const MENU_MAX_HEIGHT = 188;
const MENU_GAP = 4;

function useSelectPopoverState(triggerRef: React.RefObject<HTMLButtonElement | null>, menuRef: React.RefObject<HTMLUListElement | null>) {
  const [isOpen, setIsOpen] = useState(false);
  const [menuCoords, setMenuCoords] = useState<MenuCoords | null>(null);

  const syncMenuPosition = useCallback(() => {
    const el = triggerRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const spaceBelow = window.innerHeight - rect.bottom;
    const openUpward = spaceBelow < MENU_MAX_HEIGHT && rect.top > MENU_MAX_HEIGHT;
    setMenuCoords({ top: openUpward ? rect.top - MENU_GAP : rect.bottom + MENU_GAP, left: rect.left, width: rect.width, openUpward });
  }, [triggerRef]);

  useEffect(() => {
    if (!isOpen) return undefined;
    syncMenuPosition();
    const onOutside = (ev: MouseEvent) => {
      const target = ev.target as Node;
      if (!triggerRef.current?.contains(target) && !menuRef.current?.contains(target)) setIsOpen(false);
    };
    const onEscape = (ev: KeyboardEvent) => { if (ev.key === "Escape") { ev.stopPropagation(); setIsOpen(false); } };
    document.addEventListener("mousedown", onOutside);
    document.addEventListener("keydown", onEscape, true);
    window.addEventListener("resize", syncMenuPosition);
    return () => { document.removeEventListener("mousedown", onOutside); document.removeEventListener("keydown", onEscape, true); window.removeEventListener("resize", syncMenuPosition); };
  }, [isOpen, menuRef, syncMenuPosition, triggerRef]);

  return { isOpen, setIsOpen, menuCoords };
}

const CowokSelectMenuList: React.FC<{
  menuRef: React.RefObject<HTMLUListElement | null>;
  coords: MenuCoords;
  ariaLabel?: string;
  testId?: string;
  value: string;
  highlightedIndex: number;
  options: CowokSelectOption[];
  onHighlight: (idx: number) => void;
  onSelect: (val: string) => void;
}> = ({ menuRef, coords, ariaLabel, testId, value, highlightedIndex, options, onHighlight, onSelect }) => {
  const listNode = (
    <ul
      ref={menuRef}
      role="listbox"
      aria-label={ariaLabel}
      className="cowok-select__menu"
      style={{
        position: "fixed",
        left: `${coords.left}px`,
        width: `${coords.width}px`,
        ...(coords.openUpward ? { bottom: `${window.innerHeight - coords.top}px`, top: "auto" } : { top: `${coords.top}px`, bottom: "auto" }),
      }}
    >
      {options.map((opt, idx) => {
        const isSelected = opt.value === value;
        const isHighlighted = idx === highlightedIndex;
        return (
          <li key={opt.value} role="presentation">
            <button
              type="button"
              role="option"
              aria-selected={isSelected}
              data-testid={testId ? `${testId}-option-${opt.value}` : undefined}
              onMouseEnter={() => onHighlight(idx)}
              onClick={() => onSelect(opt.value)}
              className={`cowok-select__option${isSelected || isHighlighted ? " cowok-select__option--selected" : ""}`}
            >
              <span className="cowok-select__option-label">{opt.label}</span>
              {isSelected && <BrandIcon name="check" size={CHECK_SIZE} color="var(--action-primary)" />}
            </button>
          </li>
        );
      })}
    </ul>
  );
  return typeof document !== "undefined" ? createPortal(listNode, document.body) : listNode;
};

function resolveNextOptionIndex(key: string, current: number, total: number): number | null {
  if (key === "ArrowDown") return (current + 1) % total;
  if (key === "ArrowUp") return (current - 1 + total) % total;
  if (key === "Home") return 0;
  if (key === "End") return total - 1;
  return null;
}

function handleSelectKeyboardNav(args: {
  ev: React.KeyboardEvent<HTMLButtonElement>;
  isOpen: boolean;
  setIsOpen: React.Dispatch<React.SetStateAction<boolean>>;
  highlightedIndex: number;
  setHighlightedIndex: React.Dispatch<React.SetStateAction<number>>;
  options: CowokSelectOption[];
  onSelect: (val: string) => void;
}) {
  const { ev, isOpen, setIsOpen, highlightedIndex, setHighlightedIndex, options, onSelect } = args;
  if (options.length === 0) return;
  const nextIdx = resolveNextOptionIndex(ev.key, highlightedIndex, options.length);
  if (nextIdx !== null) {
    ev.preventDefault();
    if (!isOpen) {
      setHighlightedIndex(nextIdx);
      setIsOpen(true);
    } else {
      setHighlightedIndex(nextIdx);
    }
    return;
  }
  if (!isOpen) return;
  if (ev.key === "Enter" || ev.key === " ") {
    ev.preventDefault();
    if (options[highlightedIndex]) onSelect(options[highlightedIndex]!.value);
  } else if (ev.key === "Tab") {
    setIsOpen(false);
  }
}

export const CowokSelect: React.FC<CowokSelectProps> = ({ id, testId, value, options, onChange, ariaLabel, placeholder }) => {
  const { t } = useTranslation();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLUListElement>(null);
  const { isOpen, setIsOpen, menuCoords } = useSelectPopoverState(triggerRef, menuRef);
  const selectedIndex = Math.max(0, options.findIndex((opt) => opt.value === value));
  const [highlightedIndex, setHighlightedIndex] = useState(selectedIndex);
  const selectedOption = options.find((opt) => opt.value === value) ?? options[0];
  const resolvedAriaLabel = ariaLabel ?? t("chat.selectOptionPrompt", "Please choose an option");
  const selectAndRestoreFocus = (val: string) => {
    onChange(val);
    setIsOpen(false);
    triggerRef.current?.focus();
  };

  return (
    <div className="cowok-select">
      <button
        ref={triggerRef}
        id={id}
        type="button"
        role="combobox"
        aria-expanded={isOpen}
        aria-haspopup="listbox"
        aria-label={resolvedAriaLabel}
        data-testid={testId}
        onClick={() => {
          setHighlightedIndex(selectedIndex);
          setIsOpen((prev) => !prev);
        }}
        onKeyDown={(ev) => handleSelectKeyboardNav({ ev, isOpen, setIsOpen, highlightedIndex, setHighlightedIndex, options, onSelect: selectAndRestoreFocus })}
        className={`cowok-select__trigger${isOpen ? " cowok-select__trigger--open" : ""}`}
      >
        <span className="cowok-select__label">{selectedOption?.label ?? placeholder ?? ""}</span>
        <span className={`cowok-select__chevron${isOpen ? " cowok-select__chevron--open" : ""}`} aria-hidden="true">
          <BrandIcon name="chevron-down" size={CHEVRON_SIZE} color="currentColor" />
        </span>
      </button>
      {isOpen && menuCoords && (
        <CowokSelectMenuList menuRef={menuRef} coords={menuCoords} ariaLabel={resolvedAriaLabel} testId={testId} value={value} highlightedIndex={highlightedIndex} options={options} onHighlight={setHighlightedIndex} onSelect={selectAndRestoreFocus} />
      )}
    </div>
  );
};
