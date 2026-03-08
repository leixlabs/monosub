import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "@/lib/utils"
import { Slot } from "@radix-ui/react-slot"

const glassButtonVariants = cva(
  cn(
    "relative inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-xl cursor-pointer",
    "text-sm font-medium transition-all duration-300 ease-out",
    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/30 focus-visible:ring-offset-2 focus-visible:ring-offset-transparent",
    "disabled:pointer-events-none disabled:opacity-50",
    "hover:scale-105 active:scale-95",
    "[&_svg]:pointer-events-none [&_svg]:size-4 [&_svg]:shrink-0",
  ),
  {
    variants: {
      variant: {
        // 适用于深色/彩色背景（如 FileItem 卡片上）
        default: cn(
          "bg-white/20 dark:bg-white/20 backdrop-blur-xl border border-white/30 dark:border-white/30",
          "text-text-primary dark:text-white",
          "shadow-[0_4px_16px_rgba(0,0,0,0.1)] dark:shadow-[0_4px_16px_rgba(0,0,0,0.2)]",
          "hover:bg-white/40 dark:hover:bg-white/30 hover:border-white/50 dark:hover:border-white/40",
          "before:absolute before:inset-0 before:rounded-xl",
          "before:bg-gradient-to-b before:from-white/20 before:to-transparent before:pointer-events-none",
        ),
        // 蓝紫渐变主要行动按钮（Toolbar 添加文件、设置中验证等）
        primary: cn(
          "bg-gradient-to-r from-cyan-500/80 via-blue-500/80 to-purple-500/80",
          "backdrop-blur-xl border border-white/30 text-white",
          "shadow-[0_4px_20px_rgba(59,130,246,0.4)]",
          "hover:shadow-[0_4px_30px_rgba(59,130,246,0.6)]",
          "before:absolute before:inset-0 before:rounded-xl",
          "before:bg-gradient-to-b before:from-white/30 before:to-transparent before:pointer-events-none",
        ),
        // 带边框的次要按钮（亮暗双模式可见）
        outline: cn(
          "bg-transparent backdrop-blur-sm",
          "border border-border dark:border-white/30",
          "text-text-primary dark:text-white",
          "hover:bg-black/5 dark:hover:bg-white/10",
          "hover:border-border dark:hover:border-white/50",
        ),
        // 无背景轻量按钮（亮暗双模式可见）
        ghost: cn(
          "bg-transparent",
          "text-text-secondary dark:text-white/70",
          "hover:bg-black/5 dark:hover:bg-white/10",
          "hover:text-text-primary dark:hover:text-white",
        ),
        // 专为底部工具栏/分段控件设计：自适应亮暗模式，明显可读
        muted: cn(
          "bg-transparent",
          "text-text-secondary dark:text-gray-400",
          "hover:bg-bg-tertiary dark:hover:bg-zinc-800",
          "hover:text-text-primary dark:hover:text-white",
          "rounded-md",
        ),
        // 危险操作（取消、删除）
        destructive: cn(
          "bg-red-500/20 dark:bg-red-500/30 backdrop-blur-xl",
          "border border-red-300/40 dark:border-red-400/40",
          "text-red-600 dark:text-red-100",
          "shadow-[0_4px_16px_rgba(239,68,68,0.15)] dark:shadow-[0_4px_16px_rgba(239,68,68,0.3)]",
          "hover:bg-red-500/30 dark:hover:bg-red-500/40",
          "before:absolute before:inset-0 before:rounded-xl",
          "before:bg-gradient-to-b before:from-white/10 before:to-transparent before:pointer-events-none",
        ),
      },
      size: {
        default: "h-10 px-4 py-2",
        sm: "h-8 px-3 text-xs",
        lg: "h-12 px-6 text-base",
        icon: "h-10 w-10",
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
  glowEffect?: boolean,
  asChild?: boolean
}

const GlassButton = React.forwardRef<HTMLButtonElement, GlassButtonProps>(
  ({ className, variant, asChild = false, size, glowEffect = false, children, ...props }, ref) => {
    const Comp = asChild ? Slot : 'button'

    // 只有需要 glow 特效时才包裹外层 div
    // 否则直接返回 button，保证 w-full / flex 等布局正常工作
    if (!glowEffect) {
      return (
        <Comp className={cn(glassButtonVariants({ variant, size, className }))} ref={ref} {...props}>
          <span className="relative z-10 flex items-center gap-2">{children}</span>
        </Comp>
      )
    }

    return (
      <div className="relative inline-block">
        <div className="absolute -inset-1 rounded-xl bg-gradient-to-r from-cyan-500/40 via-blue-500/40 to-purple-500/40 blur-lg opacity-70 transition-opacity group-hover:opacity-100" />
        <Comp className={cn(glassButtonVariants({ variant, size, className }))} ref={ref} {...props}>
          <span className="relative z-10 flex items-center gap-2">{children}</span>
        </Comp>
      </div>
    )
  },
)
GlassButton.displayName = "GlassButton"

export { GlassButton, glassButtonVariants }
