import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { Check, ChevronDown } from 'lucide-react';
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
}

export function CustomSelect<T extends string = string>({
  value,
  options,
  onChange,
  className,
  triggerClassName,
  menuClassName,
  disabled = false,
}: CustomSelectProps<T>) {
  const [isOpen, setIsOpen] = useState(false);
  const [highlightedIndex, setHighlightedIndex] = useState(-1);
  const rootRef = useRef<HTMLDivElement>(null);
  const listId = useId();

  const selectedIndex = useMemo(() => options.findIndex((opt) => opt.value === value), [options, value]);
  const selectedOption = selectedIndex >= 0 ? options[selectedIndex] : undefined;

  useEffect(() => {
    if (!isOpen) {
      return;
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
  }, [isOpen]);

  const openMenu = () => {
    if (disabled || options.length === 0) {
      return;
    }
    setHighlightedIndex(selectedIndex >= 0 ? selectedIndex : 0);
    setIsOpen(true);
  };

  const handleTriggerKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>) => {
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
      setIsOpen(false);
      return;
    }

    if (event.key === 'ArrowDown') {
      if (options.length === 0) {
        return;
      }
      event.preventDefault();
      setHighlightedIndex((prev) => (prev + 1) % options.length);
      return;
    }

    if (event.key === 'ArrowUp') {
      if (options.length === 0) {
        return;
      }
      event.preventDefault();
      setHighlightedIndex((prev) => (prev - 1 + options.length) % options.length);
      return;
    }

    if ((event.key === 'Enter' || event.key === ' ') && highlightedIndex >= 0) {
      event.preventDefault();
      const option = options[highlightedIndex];
      if (option) {
        onChange(option.value);
      }
      setIsOpen(false);
    }
  };

  return (
    <div ref={rootRef} className={cn('relative', className)}>
      <button
        type="button"
        role="combobox"
        aria-expanded={isOpen}
        aria-controls={listId}
        disabled={disabled}
        onClick={() => (isOpen ? setIsOpen(false) : openMenu())}
        onKeyDown={handleTriggerKeyDown}
        className={cn(
          'w-full px-3 py-2 text-sm bg-bg-secondary dark:bg-zinc-800 border border-border dark:border-zinc-700 rounded-xl text-text-primary dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/20 shadow-sm font-medium',
          'flex items-center justify-between gap-2 transition-colors',
          disabled
            ? 'opacity-50 cursor-not-allowed'
            : 'hover:bg-white/80 dark:hover:bg-zinc-700/70',
          triggerClassName,
        )}
      >
        <span className="truncate">{selectedOption?.label ?? ''}</span>
        <ChevronDown
          className={cn('w-4 h-4 shrink-0 text-text-secondary dark:text-gray-400 transition-transform', isOpen && 'rotate-180')}
        />
      </button>

      {isOpen && (
        <div
          id={listId}
          role="listbox"
          className={cn(
            'absolute right-0 mt-2 w-full z-30 p-1 rounded-xl border border-border dark:border-zinc-700',
            'bg-white/95 dark:bg-zinc-900/95 backdrop-blur-xl shadow-glass-lg',
            'max-h-64 overflow-y-auto',
            menuClassName,
          )}
        >
          {options.map((option, index) => {
            const isSelected = option.value === value;
            const isHighlighted = index === highlightedIndex;

            return (
              <button
                type="button"
                key={option.value}
                role="option"
                aria-selected={isSelected}
                onMouseEnter={() => setHighlightedIndex(index)}
                onClick={() => {
                  onChange(option.value);
                  setIsOpen(false);
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
                <span className="truncate">{option.label}</span>
                {isSelected && <Check className="w-4 h-4 text-primary" />}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
