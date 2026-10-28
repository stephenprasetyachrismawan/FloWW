import * as SelectPrimitive from "@radix-ui/react-select";
import { Check, ChevronDown } from "lucide-react";
import * as React from "react";

import { cn } from "@/lib/utils";

export const Select = SelectPrimitive.Root;
export const SelectValue = SelectPrimitive.Value;

export function SelectTrigger({
  className,
  children,
  ...props
}: React.ComponentProps<typeof SelectPrimitive.Trigger>) {
  return (
    <SelectPrimitive.Trigger
      className={cn(
        "flex h-[38px] w-full items-center gap-2 rounded-lg border border-line-2 bg-surface pr-2.5 pl-3 text-left text-ink",
        "hover:border-faint data-[state=open]:border-ink cursor-pointer [&>span]:truncate",
        className,
      )}
      {...props}
    >
      <span className="flex-1 truncate">{children}</span>
      <SelectPrimitive.Icon asChild>
        <ChevronDown className="size-4 text-faint" />
      </SelectPrimitive.Icon>
    </SelectPrimitive.Trigger>
  );
}

export function SelectContent({
  className,
  children,
  position = "popper",
  ...props
}: React.ComponentProps<typeof SelectPrimitive.Content>) {
  return (
    <SelectPrimitive.Portal>
      <SelectPrimitive.Content
        position={position}
        sideOffset={6}
        className={cn(
          "z-50 max-h-72 min-w-[var(--radix-select-trigger-width)] overflow-hidden rounded-[10px] border border-line bg-surface shadow-pop",
          className,
        )}
        {...props}
      >
        <SelectPrimitive.Viewport className="p-1.5">{children}</SelectPrimitive.Viewport>
      </SelectPrimitive.Content>
    </SelectPrimitive.Portal>
  );
}

/** An option with an optional one-line description under the label. */
export function SelectItem({
  className,
  children,
  description,
  ...props
}: React.ComponentProps<typeof SelectPrimitive.Item> & { description?: string }) {
  return (
    <SelectPrimitive.Item
      className={cn(
        "relative flex cursor-pointer items-start gap-2.5 rounded-[7px] py-2 pr-2.5 pl-2.5 text-text outline-none select-none",
        "data-[highlighted]:bg-sunken data-[highlighted]:text-ink data-[state=checked]:font-semibold data-[state=checked]:text-ink",
        className,
      )}
      {...props}
    >
      <span className="w-4 pt-0.5 text-accent">
        <SelectPrimitive.ItemIndicator>
          <Check className="size-4" />
        </SelectPrimitive.ItemIndicator>
      </span>
      <span className="min-w-0 flex-1">
        <SelectPrimitive.ItemText>{children}</SelectPrimitive.ItemText>
        {description ? <span className="block text-xs font-normal text-faint">{description}</span> : null}
      </span>
    </SelectPrimitive.Item>
  );
}
