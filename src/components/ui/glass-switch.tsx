"use client"

import * as React from "react"
import * as SwitchPrimitive from "@radix-ui/react-switch"
import { cn } from "@/lib/utils"

/**
 * Toggle. Off = neutral gray track, On = solid brand track; the thumb always
 * stays white so the state is readable by position *and* colour. 40×22 with a
 * 18px thumb (previously 44×24 with a glowing cyan track).
 */
const GlassSwitch = React.forwardRef<
  React.ElementRef<typeof SwitchPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof SwitchPrimitive.Root>
>(({ className, ...props }, ref) => (
  <SwitchPrimitive.Root
    className={cn(
      "peer inline-flex h-[22px] w-10 shrink-0 cursor-pointer items-center rounded-full border border-transparent",
      "transition-colors duration-200 ease-standard",
      "bg-black/20 dark:bg-white/20",
      "hover:bg-black/25 dark:hover:bg-white/25",
      "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/50 focus-visible:ring-offset-2",
      "focus-visible:ring-offset-bg-secondary dark:focus-visible:ring-offset-zinc-950",
      "disabled:cursor-not-allowed disabled:opacity-45",
      "data-[state=checked]:bg-primary data-[state=checked]:hover:bg-primary-hover",
      className,
    )}
    {...props}
    ref={ref}
  >
    <SwitchPrimitive.Thumb
      className={cn(
        "pointer-events-none block h-[18px] w-[18px] rounded-full bg-white",
        "shadow-[0_1px_3px_rgba(0,0,0,0.3)]",
        "transition-transform duration-200 ease-out-expo",
        "data-[state=checked]:translate-x-[20px] data-[state=unchecked]:translate-x-[2px]",
      )}
    />
  </SwitchPrimitive.Root>
))
GlassSwitch.displayName = SwitchPrimitive.Root.displayName

export { GlassSwitch }
