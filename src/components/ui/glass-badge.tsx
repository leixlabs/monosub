import type * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "@/lib/utils"

const glassBadgeVariants = cva(
  cn(
    "inline-flex items-center rounded-full px-3 py-1 text-xs font-medium",
    "backdrop-blur-xl border transition-all duration-300",
  ),
  {
    variants: {
      // Every variant carries a light/dark pair. The badges sit inside a
      // `GlassCard` (`bg-white/10`), which is translucent over whatever the page
      // is — light in light mode. A white-on-white pair (`text-white` over
      // `bg-white/15`) is therefore invisible in light mode, which is exactly
      // what "这个底色看不清文字" reported: the tint was there, the text was not.
      // The rule is `GlassButton`'s: a light-mode token for the fill and the
      // text, and the white/100-level colour only under `dark:`.
      variant: {
        default: cn(
          "bg-black/[0.06] dark:bg-white/15",
          "border-black/10 dark:border-white/25",
          "text-text-primary dark:text-white",
        ),
        primary: cn(
          "bg-primary/10 dark:bg-gradient-to-r dark:from-cyan-500/30 dark:to-blue-500/30",
          "border-primary/25 dark:border-cyan-400/30",
          "text-primary dark:text-cyan-100",
        ),
        success: cn(
          "bg-emerald-500/10 dark:bg-emerald-500/20",
          "border-emerald-500/25 dark:border-emerald-400/30",
          "text-emerald-700 dark:text-emerald-100",
        ),
        warning: cn(
          "bg-amber-500/10 dark:bg-amber-500/20",
          "border-amber-500/25 dark:border-amber-400/30",
          "text-amber-700 dark:text-amber-100",
        ),
        destructive: cn(
          "bg-red-500/10 dark:bg-red-500/20",
          "border-red-500/25 dark:border-red-400/30",
          "text-red-600 dark:text-red-100",
        ),
        outline: cn(
          "bg-transparent",
          "border-border dark:border-white/30",
          "text-text-secondary dark:text-white/80",
        ),
      },
      size: {
        sm: "px-2 py-0.5 text-xs",
        md: "px-3 py-1 text-sm",
        lg: "px-4 py-2 text-base",
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
  // `size` must reach the variant call: spreading it instead put `size="sm"` on
  // the div, where it is a valid HTML attribute with no effect, so every badge in
  // the app silently rendered at `md` (px-3 py-1) and each caller's `sm` was a
  // no-op. That is what made the file row taller than its content needed.
  return <div className={cn(glassBadgeVariants({ variant, size }), className)} {...props} />
}

export { GlassBadge, glassBadgeVariants }
