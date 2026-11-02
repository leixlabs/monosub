"use client"

import * as React from "react"
import { cn } from "@/lib/utils"

export interface GlassInputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  /** Kept for API compatibility; focus is now a crisp ring instead of a blurred glow. */
  glowOnFocus?: boolean
  /** Renders the error border/ring. Pair with `aria-invalid` + a message. */
  invalid?: boolean
}

/**
 * Text input. Theme-aware surface (the old one was hard-coded white-on-white and
 * unreadable in light mode), hairline border, 3px brand focus ring, explicit
 * invalid and disabled states.
 */
const GlassInput = React.forwardRef<HTMLInputElement, GlassInputProps>(
  ({ className, type, glowOnFocus, invalid, ...props }, ref) => {
    // Destroyed on purpose: not a valid HTML attribute, so it must not reach
    // the DOM through `...props`.
    void glowOnFocus;
    return (
      <input
        type={type}
        aria-invalid={invalid || props["aria-invalid"]}
        className={cn(
          "flex h-9 w-full rounded-lg px-3 py-2 text-sm",
          "bg-surface text-text-primary dark:text-zinc-100",
          "placeholder:text-text-tertiary dark:placeholder:text-zinc-500",
          "border border-line-strong shadow-card",
          "transition-[border-color,box-shadow,background-color] duration-200 ease-standard",
          "hover:border-black/25 dark:hover:border-white/25",
          "focus:outline-none focus:border-primary focus:shadow-ring-primary",
          "disabled:cursor-not-allowed disabled:opacity-50 disabled:bg-bg-tertiary dark:disabled:bg-white/[0.04]",
          "aria-[invalid=true]:border-danger aria-[invalid=true]:focus:shadow-[0_0_0_3px_rgba(220,53,69,0.2)]",
          "file:border-0 file:bg-transparent file:text-sm file:font-medium",
          className,
        )}
        ref={ref}
        {...props}
      />
    )
  },
)
GlassInput.displayName = "GlassInput"

export { GlassInput }
