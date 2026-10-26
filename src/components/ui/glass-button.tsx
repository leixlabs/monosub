import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Loader2 } from "lucide-react"
import { cn } from "@/lib/utils"
import { Slot } from "@radix-ui/react-slot"

/**
 * Button system.
 *
 * Hierarchy (one primary per view):
 *   primary     – solid brand fill, the single main action
 *   default     – neutral raised button, secondary actions
 *   outline     – hairline border, tertiary
 *   ghost/muted – no chrome, toolbar & row actions
 *   destructive – tinted red, stop / delete
 *
 * States: hover (bg shift, no scale), active (0.98 press), focus-visible (ring),
 * disabled (45% + no pointer), `loading` (spinner, locks width, aria-busy).
 */
const glassButtonVariants = cva(
  cn(
    "relative inline-flex items-center justify-center gap-1.5 whitespace-nowrap select-none cursor-pointer",
    "text-sm font-medium",
    "transition-[background-color,border-color,color,box-shadow,transform] duration-200 ease-standard",
    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/50 focus-visible:ring-offset-2",
    "focus-visible:ring-offset-bg-secondary dark:focus-visible:ring-offset-zinc-950",
    "disabled:pointer-events-none disabled:opacity-45",
    "aria-busy:pointer-events-none",
    "[&_svg]:pointer-events-none [&_svg]:size-4 [&_svg]:shrink-0",
  ),
  {
    variants: {
      variant: {
        default: cn(
          "bg-surface text-text-primary dark:text-zinc-100",
          "border border-line-strong shadow-card",
          "hover:bg-bg-tertiary dark:hover:bg-white/[0.06]",
          "active:bg-bg-tertiary dark:active:bg-white/10",
        ),
        primary: cn(
          "bg-primary text-primary-foreground border border-transparent",
          "shadow-[0_1px_2px_rgba(16,24,40,0.12),inset_0_1px_0_rgba(255,255,255,0.18)]",
          "hover:bg-primary-hover hover:shadow-float",
          "active:shadow-none",
        ),
        outline: cn(
          "bg-transparent text-text-primary dark:text-zinc-100",
          "border border-line-strong",
          "hover:bg-black/[0.04] dark:hover:bg-white/[0.06]",
        ),
        ghost: cn(
          "bg-transparent text-text-secondary dark:text-zinc-400",
          "hover:bg-black/[0.05] dark:hover:bg-white/[0.07]",
          "hover:text-text-primary dark:hover:text-zinc-100",
        ),
        muted: cn(
          "bg-transparent text-text-secondary dark:text-zinc-400 rounded-md",
          "hover:bg-black/[0.05] dark:hover:bg-white/[0.07]",
          "hover:text-text-primary dark:hover:text-zinc-100",
        ),
        destructive: cn(
          "bg-danger/10 text-danger dark:text-red-300 dark:bg-red-500/15",
          "border border-danger/25 dark:border-red-400/25",
          "hover:bg-danger/15 dark:hover:bg-red-500/25",
        ),
      },
      size: {
        default: "h-9 px-4 rounded-lg",
        sm: "h-8 px-3 text-xs rounded-lg",
        lg: "h-11 px-6 text-base rounded-xl",
        icon: "h-9 w-9 rounded-lg",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
)

export interface GlassButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
  VariantProps<typeof glassButtonVariants> {
  /** Kept for API compatibility; the rainbow glow wrapper is gone. */
  glowEffect?: boolean,
  asChild?: boolean
  /** Shows a spinner, sets aria-busy and blocks clicks. */
  loading?: boolean
}

const GlassButton = React.forwardRef<HTMLButtonElement, GlassButtonProps>(
  // `glowEffect` is accepted and ignored so existing call sites keep compiling.
  ({ className, variant, asChild = false, size, glowEffect: _glow, loading = false, disabled, children, ...props }, ref) => {
    const Comp = asChild ? Slot : 'button'

    // Slot requires exactly one child, so the spinner is only injected for real buttons.
    return (
      <Comp
        className={cn(glassButtonVariants({ variant, size, className }))}
        ref={ref}
        disabled={disabled || loading}
        aria-busy={loading || undefined}
        {...props}
      >
        {asChild ? children : (
          <>
            {loading && <Loader2 className="animate-spin" aria-hidden="true" />}
            {children}
          </>
        )}
      </Comp>
    )
  },
)
GlassButton.displayName = "GlassButton"

export { GlassButton, glassButtonVariants }
