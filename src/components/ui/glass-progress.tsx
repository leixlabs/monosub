"use client"

import * as React from "react"
import * as ProgressPrimitive from "@radix-ui/react-progress"
import { cn } from "@/lib/utils"

/**
 * Slim determinate progress bar: 6px track, solid brand fill, a moving sheen while
 * running. The old 12px bar with a three-colour gradient and a blurred halo drew
 * more attention than the file name above it.
 */
const GlassProgress = React.forwardRef<
  React.ElementRef<typeof ProgressPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof ProgressPrimitive.Root>
>(({ className, value, ...props }, ref) => {
  const pct = Math.min(100, Math.max(0, value || 0))
  return (
    <ProgressPrimitive.Root
      ref={ref}
      value={pct}
      className={cn(
        "relative h-1.5 w-full overflow-hidden rounded-full",
        "bg-black/[0.07] dark:bg-white/10",
        className,
      )}
      {...props}
    >
      <ProgressPrimitive.Indicator
        className={cn(
          "relative h-full rounded-full bg-primary",
          "transition-[width] duration-500 ease-out-expo",
          // Sheen: reads as "alive" without a second number or a glow.
          "after:absolute after:inset-0 after:bg-[linear-gradient(90deg,transparent,rgba(255,255,255,0.35),transparent)]",
          "after:bg-[length:200%_100%] after:animate-shimmer motion-reduce:after:hidden",
        )}
        style={{ width: `${pct}%` }}
      />
    </ProgressPrimitive.Root>
  )
})
GlassProgress.displayName = ProgressPrimitive.Root.displayName

export { GlassProgress }
