import * as React from "react"
import { cn } from "@/lib/utils"

interface GlassCardProps extends React.HTMLAttributes<HTMLDivElement> {
  /**
   * "Active" emphasis. Kept under the old prop name so callers are unchanged, but
   * it no longer paints a blurred rainbow halo: an active card gets a brand-coloured
   * hairline ring and a slightly deeper shadow. It reads as "this one is working"
   * without bleeding colour onto neighbouring cards.
   */
  glowEffect?: boolean
  /** Lifts on hover. Off for cards that already carry motion (e.g. progress). */
  interactive?: boolean
  children: React.ReactNode
}

const GlassCard = React.forwardRef<HTMLDivElement, GlassCardProps>(
  ({ className, glowEffect = false, interactive = false, children, ...props }, ref) => {
    return (
      <div
        ref={ref}
        data-active={glowEffect || undefined}
        className={cn(
          "relative rounded-2xl border bg-surface",
          "border-line shadow-card",
          "transition-[box-shadow,border-color,transform] duration-250 ease-standard",
          interactive && "hover:-translate-y-px hover:shadow-card-hover hover:border-line-strong",
          // Active: brand ring instead of a coloured blur.
          "data-[active=true]:border-primary/40 data-[active=true]:shadow-[0_0_0_3px_rgb(var(--primary)/0.10),0_4px_12px_rgba(16,24,40,0.08)]",
          className,
        )}
        {...props}
      >
        {children}
      </div>
    )
  },
)
GlassCard.displayName = "GlassCard"

const GlassCardHeader = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => <div ref={ref} className={cn("flex flex-col gap-1.5 p-5", className)} {...props} />,
)
GlassCardHeader.displayName = "GlassCardHeader"

const GlassCardTitle = React.forwardRef<HTMLHeadingElement, React.HTMLAttributes<HTMLHeadingElement>>(
  ({ className, ...props }, ref) => (
    <h3
      ref={ref}
      className={cn("text-lg font-semibold text-text-primary dark:text-zinc-50 leading-tight", className)}
      {...props}
    />
  ),
)
GlassCardTitle.displayName = "GlassCardTitle"

const GlassCardDescription = React.forwardRef<HTMLParagraphElement, React.HTMLAttributes<HTMLParagraphElement>>(
  ({ className, ...props }, ref) => (
    <p ref={ref} className={cn("text-sm text-text-secondary dark:text-zinc-400", className)} {...props} />
  ),
)
GlassCardDescription.displayName = "GlassCardDescription"

const GlassCardContent = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => <div ref={ref} className={cn("p-5 pt-0", className)} {...props} />,
)
GlassCardContent.displayName = "GlassCardContent"

const GlassCardFooter = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("flex items-center p-5 pt-0", className)} {...props} />
  ),
)
GlassCardFooter.displayName = "GlassCardFooter"

export { GlassCard, GlassCardHeader, GlassCardTitle, GlassCardDescription, GlassCardContent, GlassCardFooter }
