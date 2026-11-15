import type { ReactNode } from "react";
import { cn } from "@cloudflare/kumo/utils";

/**
 * Standard dialog anatomy: fixed header, independently scrolling body, fixed footer.
 * Use with `<Dialog className={DIALOG_FRAME}>` so Kumo's own padding doesn't double up.
 * Nothing ever overlaps: the footer sits outside the scroll area.
 */
export const DIALOG_FRAME = "flex max-h-[min(88dvh,56rem)] flex-col overflow-hidden p-0";

interface DialogFrameProps {
  header?: ReactNode;
  footer?: ReactNode;
  children: ReactNode;
  /** Fill the dialog's height (for logs/terminals) instead of sizing to content. */
  fill?: boolean;
}

export function DialogFrame({ header, footer, children, fill }: DialogFrameProps) {
  return (
    <>
      {header ? <div className="shrink-0 px-6 pb-4 pt-6">{header}</div> : null}
      <div className={cn("min-h-0 flex-1 overflow-y-auto overscroll-contain px-6 pb-6", !header && "pt-6", fill && "flex flex-col")}>{children}</div>
      {footer ? <div className="flex shrink-0 flex-wrap items-center justify-end gap-2 border-t border-kumo-hairline bg-kumo-base px-6 py-4">{footer}</div> : null}
    </>
  );
}
