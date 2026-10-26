import type * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "@/lib/utils"

/**
 * Status badge. Tinted background + same-hue text, each with an explicit
 * light/dark pair so contrast holds on any surface (no more white-on-white).
 * Radius is `md` rather than a full pill: status chips read as labels, not buttons.
 */
const glassBadgeVariants = cva(
  cn(
    "inline-flex items-center rounded-md px-2 py-0.5 text-xs font-medium leading-5",
    "border transition-colors duration-200",
  ),
  {
    variants: {
      variant: {
        default: cn(
          "bg-black/[0.05] dark:bg-white/[0.08]",
          "border-transparent",
          "text-text-secondary dark:text-zinc-300",
        ),
        primary: cn(
          "bg-primary/10 dark:bg-primary/15",
          "border-primary/20",
          "text-primary",
        ),
        success: cn(
          "bg-emerald-500/10 dark:bg-emerald-400/15",
          "border-emerald-600/20 dark:border-emerald-400/25",
          "text-emerald-700 dark:text-emerald-300",
        ),
        warning: cn(
          "bg-amber-500/10 dark:bg-amber-400/15",
          "border-amber-600/25 dark:border-amber-400/25",
          "text-amber-700 dark:text-amber-300",
        ),
        destructive: cn(
          "bg-red-500/10 dark:bg-red-400/15",
          "border-red-600/20 dark:border-red-400/25",
          "text-red-700 dark:text-red-300",
        ),
        outline: cn(
          "bg-transparent",
          "border-line-strong",
          "text-text-secondary dark:text-zinc-300",
        ),
      },
      size: {
        sm: "px-2 py-0.5 text-xs",
        md: "px-2.5 py-0.5 text-sm",
        lg: "px-3 py-1 text-base",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "md",
    },
  },
)

export interface GlassBadgeProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof glassBadgeVariants> {}

function GlassBadge({ className, variant, size, ...props }: GlassBadgeProps) {
  return <div className={cn(glassBadgeVariants({ variant, size }), className)} {...props} />
}

export { GlassBadge, glassBadgeVariants }
