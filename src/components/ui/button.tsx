import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import * as React from "react";

import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap font-semibold transition-colors disabled:pointer-events-none disabled:opacity-40 cursor-pointer [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        /** The single main action on a screen. */
        primary: "rounded-lg bg-accent text-accent-ink hover:brightness-105",
        default: "rounded-lg border border-line-2 bg-surface text-ink hover:bg-sunken",
        /** Quiet text action. */
        link: "text-muted hover:text-ink",
      },
      size: {
        default: "h-9 px-3.5 text-[14.5px]",
        sm: "h-[30px] px-2.5 text-[13px]",
        none: "text-[13.5px]",
      },
    },
    defaultVariants: { variant: "default", size: "default" },
  },
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

export function Button({ className, variant, size, asChild = false, ...props }: ButtonProps) {
  const Comp = asChild ? Slot : "button";
  const resolvedSize = variant === "link" && !size ? "none" : size;
  return <Comp className={cn(buttonVariants({ variant, size: resolvedSize }), className)} {...props} />;
}
