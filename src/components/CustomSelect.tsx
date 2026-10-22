import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { Check, ChevronDown, Search } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/utils';

export interface SelectOption<T extends string = string> {
  value: T;
  label: string;
}

interface CustomSelectProps<T extends string = string> {
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  className?: string;
  triggerClassName?: string;
  menuClassName?: string;
  disabled?: boolean;
  /**
   * `data-testid` for the trigger button, so a pilot scenario can address this
   * select without a positional selector (`tests/pilot/*.toml`).
   */
  testId?: string;
  /**
   * Show a filter box at the top of the menu.
   *
   * Off by default so the short lists (output language, target language, the
   * plan's three profile answers) keep their one-click shape. On for the model
   * pickers, where the catalogue offers 82 recognition and 40 translation
   * models: scrolling that list to find one id is the actual complaint, and a
   * filter is the only thing that scales as the catalogue grows.
   */
  searchable?: boolean;
}

export function CustomSelect<T extends string = string>({
  value,
  options,
  onChange,
  className,
  triggerClassName,
  menuClassName,
  disabled = false,
  testId,
  searchable = false,
}: CustomSelectProps<T>) {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [highlightedIndex, setHighlightedIndex] = useState(-1);
  const [query, setQuery] = useState('');
  const rootRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const listId = useId();

  const selectedIndex = useMemo(() => options.findIndex((opt) => opt.value === value), [options, value]);
  const selectedOption = selectedIndex >= 0 ? options[selectedIndex] : undefined;

  /**
   * The options the menu actually renders, with their index in `options`.
   *
   * Matching is case-insensitive and substring-based, not prefix-based: a user
   * looking for `sensevoice_small_q8` types `sense` or `q8`, and both must find
   * it.
   *
   * The empty-value option (`Auto`) is kept **only when something else matches**:
   * it is the way to clear a choice, so hiding it entirely would leave an
   * override impossible to undo. But keeping it unconditionally made a search
   * with no hits render a lone "Auto" row and no "no matches" hint — the user
   * then reads the filter as broken rather than as finding nothing.
   */
  const visibleOptions = useMemo(() => {
    const all = options.map((option, index) => ({ option, index }));
    if (!searchable) {
      return all;
    }
    const needle = query.trim().toLowerCase();
    if (!needle) {
      return all;
    }
    const matches = all.filter(({ option }) => option.label.toLowerCase().includes(needle));
    if (matches.length > 0) {
      // Keep `Auto` reachable at the top so a search never traps the user in a
      // filtered list they cannot clear.
      const clear = all.find(({ option }) => option.value === '');
      return clear && !matches.includes(clear) ? [clear, ...matches] : matches;
    }
    return matches;
  }, [options, query, searchable]);

  /**
   * Move the highlight onto the first row a query leaves visible.
   *
   * Done from the change event, **not** from a `useEffect` on `visibleOptions`.
   * An effect that writes `highlightedIndex` whenever its dependencies change
   * also fires after the re-render that an ArrowDown causes, so it snapped the
   * highlight back to the top row on every keypress — measured: three ArrowDown
   * presses left the first row highlighted. Highlight is UI state driven by
   * events; deriving it in an effect fights the keyboard handler.
   */
  const handleQueryChange = (next: string) => {
    setQuery(next);
    const needle = next.trim().toLowerCase();
    if (!needle) {
      setHighlightedIndex(selectedIndex >= 0 ? selectedIndex : 0);
      return;
    }
    const first = options.findIndex((option) => option.value !== '' && option.label.toLowerCase().includes(needle));
    setHighlightedIndex(first >= 0 ? first : -1);
  };

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    // The filter box takes focus on open, so typing filters immediately instead
    // of requiring a click into the field first.
    if (searchable) {
      searchRef.current?.focus();
    }

    const handleOutsideClick = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setIsOpen(false);
      }
    };

    document.addEventListener('mousedown', handleOutsideClick);
    document.addEventListener('keydown', handleEscape);

    return () => {
      document.removeEventListener('mousedown', handleOutsideClick);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [isOpen, searchable]);

  const closeMenu = () => {
    setIsOpen(false);
    // A stale filter would make the next open look empty and, worse, make the
    // list disagree with the trigger label.
    setQuery('');
  };

  const openMenu = () => {
    if (disabled || options.length === 0) {
      return;
    }
    setHighlightedIndex(selectedIndex >= 0 ? selectedIndex : 0);
    setQuery('');
    setIsOpen(true);
  };

  /**
   * Keyboard handling shared by the trigger and the filter box.
   *
   * A searchable menu puts focus in the input, so the input must also handle
   * arrows/Enter — otherwise typing a query and pressing Enter does nothing.
   * Highlight moves through the *visible* rows, not the full option list.
   */
  const handleKeyDown = (event: React.KeyboardEvent<HTMLElement>) => {
    if (disabled) {
      return;
    }

    if (!isOpen) {
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp' || event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        openMenu();
      }
      return;
    }

    if (event.key === 'Tab') {
      // A searchable menu opened by click leaves focus in the input; Tab is the
      // user's way out, and closing first keeps the focus ring honest.
      if (searchable) {
        closeMenu();
      } else {
        setIsOpen(false);
      }
      return;
    }

    if (visibleOptions.length === 0) {
      return;
    }

    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const step = event.key === 'ArrowDown' ? 1 : -1;
      setHighlightedIndex((prev) => {
        const position = visibleOptions.findIndex((entry) => entry.index === prev);
        if (position === -1) {
          return visibleOptions[0].index;
        }
        const next = (position + step + visibleOptions.length) % visibleOptions.length;
        return visibleOptions[next].index;
      });
      return;
    }

    if (event.key === 'Enter') {
      event.preventDefault();
      const entry = visibleOptions.find((candidate) => candidate.index === highlightedIndex);
      if (entry) {
        onChange(entry.option.value);
        closeMenu();
      }
    }
  };

  return (
    <div ref={rootRef} className={cn('relative', className)}>
      <button
        type="button"
        role="combobox"
        data-testid={testId}
        aria-expanded={isOpen}
        aria-controls={listId}
        disabled={disabled}
        onClick={() => (isOpen ? closeMenu() : openMenu())}
        onKeyDown={handleKeyDown}
        className={cn(
          'w-full px-3 py-2 text-sm bg-bg-secondary dark:bg-zinc-800 border border-border dark:border-zinc-700 rounded-xl text-text-primary dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/20 shadow-sm font-medium',
          'flex items-center justify-between gap-2 transition-colors',
          disabled
            ? 'opacity-50 cursor-not-allowed'
            : 'hover:bg-white/80 dark:hover:bg-zinc-700/70',
          triggerClassName,
        )}
      >
        <span className="truncate" title={selectedOption?.label}>{selectedOption?.label ?? ''}</span>
        <ChevronDown
          className={cn('w-4 h-4 shrink-0 text-text-secondary dark:text-gray-400 transition-transform', isOpen && 'rotate-180')}
        />
      </button>

      {isOpen && (
        <div
          id={listId}
          role="listbox"
          className={cn(
            // The menu must not inherit the trigger's (often narrow) width, or
            // long option labels are truncated to a few characters. Size it to
            // its content instead: at least the trigger, up to a readable cap.
            'absolute right-0 mt-2 z-30 p-1 rounded-xl border border-border dark:border-zinc-700',
            'min-w-full w-max max-w-[min(24rem,70vw)]',
            'bg-white/95 dark:bg-zinc-900/95 backdrop-blur-xl shadow-glass-lg',
            'max-h-64 overflow-y-auto',
            menuClassName,
          )}
        >
          {searchable && (
            // Sticky, so the filter stays reachable while scrolling a long list.
            <div className="sticky top-0 z-10 p-1 pb-2 bg-white/95 dark:bg-zinc-900/95">
              <div className="relative">
                <Search className="w-3.5 h-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 text-text-tertiary dark:text-gray-500" />
                <input
                  ref={searchRef}
                  type="text"
                  value={query}
                  onChange={(event) => handleQueryChange(event.target.value)}
                  onKeyDown={handleKeyDown}
                  placeholder={t('modelPlan.searchPlaceholder')}
                  aria-label={t('modelPlan.searchPlaceholder')}
                  data-testid={testId ? `${testId}-search` : undefined}
                  className="w-full pl-8 pr-2 py-1.5 text-sm bg-bg-secondary dark:bg-zinc-800 border border-border dark:border-zinc-700 rounded-lg text-text-primary dark:text-white placeholder:text-text-tertiary dark:placeholder:text-gray-500 focus:outline-none focus:ring-2 focus:ring-primary/20"
                />
              </div>
            </div>
          )}

          {visibleOptions.length === 0 && (
            <p className="px-3 py-2 text-sm text-text-tertiary dark:text-gray-500">
              {t('modelPlan.searchEmpty')}
            </p>
          )}

          {visibleOptions.map(({ option, index }) => {
            const isSelected = option.value === value;
            const isHighlighted = index === highlightedIndex;

            return (
              <button
                type="button"
                key={option.value}
                role="option"
                data-testid={testId ? `${testId}-option-${option.value}` : undefined}
                aria-selected={isSelected}
                onMouseEnter={() => setHighlightedIndex(index)}
                onClick={() => {
                  onChange(option.value);
                  closeMenu();
                }}
                className={cn(
                  'w-full px-3 py-2 rounded-lg text-sm text-left transition-colors',
                  'flex items-center justify-between gap-2',
                  isSelected
                    ? 'text-text-primary dark:text-white bg-primary/10 dark:bg-primary/20'
                    : 'text-text-secondary dark:text-gray-300',
                  isHighlighted && 'bg-bg-secondary dark:bg-zinc-800 text-text-primary dark:text-white',
                )}
              >
                <span className="truncate" title={option.label}>{option.label}</span>
                {isSelected && <Check className="w-4 h-4 text-primary" />}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
