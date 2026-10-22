import * as React from "react"
import * as DialogPrimitive from "@radix-ui/react-dialog"
import { X } from "lucide-react"
import { cn } from "@/lib/utils"

const GlassDialog = DialogPrimitive.Root

const GlassDialogTrigger = DialogPrimitive.Trigger

const GlassDialogPortal = DialogPrimitive.Portal

const GlassDialogClose = DialogPrimitive.Close

const GlassDialogOverlay = React.forwardRef<
  React.ElementRef<typeof DialogPrimitive.Overlay>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Overlay>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Overlay
    ref={ref}
    className={cn(
      "fixed inset-0 z-50 bg-black/60 backdrop-blur-sm",
      "data-[state=open]:animate-in data-[state=closed]:animate-out",
      "data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0",
      className,
    )}
    {...props}
  />
))
GlassDialogOverlay.displayName = DialogPrimitive.Overlay.displayName

export interface GlassDialogContentProps
  extends React.ComponentPropsWithoutRef<typeof DialogPrimitive.Content> {
  /**
   * Overrides the built-in close button's colours.
   *
   * The default surface is dark glass, so the close glyph is white/60. A dialog
   * that paints a light surface instead cannot restate that from `className`:
   * the close button is rendered here, not by the caller.
   */
  closeClassName?: string
}

const GlassDialogContent = React.forwardRef<
  React.ElementRef<typeof DialogPrimitive.Content>,
  GlassDialogContentProps
>(({ className, closeClassName, children, ...props }, ref) => (
  <GlassDialogPortal>
    <GlassDialogOverlay />
    {/*
      Centred with flexbox, not `left-1/2 top-1/2 -translate-*`.

      The translate version did not work: `tailwindcss-animate`'s `animate-in`
      keyframes set `transform` with `animation-fill-mode: both`, which overrides
      the utility's `--tw-translate-y: -50%`. The computed transform was the
      identity matrix, so `top: 50%` became the dialog's *top* edge — a 484px-tall
      dialog on an 800px viewport started at 400px and ran off the bottom, with its
      buttons cut off (measured in the running app). Flex centring needs no
      transform, so the entrance animation cannot break the layout.

      `p-4` keeps a gutter, and the max height lives on the content so a tall
      dialog scrolls inside the viewport instead of leaving it.
    */}
    <div className="pointer-events-none fixed inset-0 z-50 flex items-center justify-center p-4">
      <DialogPrimitive.Content
        ref={ref}
        className={cn(
          "pointer-events-auto relative flex max-h-full w-full max-w-lg flex-col",
          "rounded-2xl border border-white/20 p-6",
          "bg-white/10 backdrop-blur-2xl",
          "shadow-[0_8px_32px_rgba(0,0,0,0.4)]",
          "before:absolute before:inset-0 before:rounded-2xl",
          "before:bg-gradient-to-b before:from-white/15 before:to-transparent before:pointer-events-none",
          "data-[state=open]:animate-in data-[state=closed]:animate-out",
          "data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0",
          "data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95",
          className,
        )}
        {...props}
      >
        <div className="relative z-10 flex min-h-0 flex-col">{children}</div>
        <DialogPrimitive.Close
          className={cn(
            "absolute right-4 top-4 rounded-lg p-1 text-white/60 transition-all hover:text-white hover:bg-white/10 focus:outline-none focus:ring-2 focus:ring-white/50",
            closeClassName,
          )}
        >
          <X className="h-4 w-4" />
          <span className="sr-only">Close</span>
        </DialogPrimitive.Close>
      </DialogPrimitive.Content>
    </div>
  </GlassDialogPortal>
))
GlassDialogContent.displayName = DialogPrimitive.Content.displayName

const GlassDialogHeader = ({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) => (
  <div className={cn("flex flex-col gap-1.5 text-center sm:text-left", className)} {...props} />
)
GlassDialogHeader.displayName = "GlassDialogHeader"

const GlassDialogFooter = ({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) => (
  <div className={cn("flex flex-col-reverse sm:flex-row sm:justify-end gap-2 mt-6", className)} {...props} />
)
GlassDialogFooter.displayName = "GlassDialogFooter"

const GlassDialogTitle = React.forwardRef<
  React.ElementRef<typeof DialogPrimitive.Title>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Title>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Title
    ref={ref}
    className={cn("text-lg font-semibold text-white leading-none tracking-tight", className)}
    {...props}
  />
))
GlassDialogTitle.displayName = DialogPrimitive.Title.displayName

const GlassDialogDescription = React.forwardRef<
  React.ElementRef<typeof DialogPrimitive.Description>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Description>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Description ref={ref} className={cn("text-sm text-white/60", className)} {...props} />
))
GlassDialogDescription.displayName = DialogPrimitive.Description.displayName

export {
  GlassDialog,
  GlassDialogPortal,
  GlassDialogOverlay,
  GlassDialogClose,
  GlassDialogTrigger,
  GlassDialogContent,
  GlassDialogHeader,
  GlassDialogFooter,
  GlassDialogTitle,
  GlassDialogDescription,
}
