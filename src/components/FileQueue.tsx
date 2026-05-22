import { useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";

import { useWehiStore } from "../store";
import { isVideoPath, type QueueItemStatus } from "../types";

function statusBadge(status: QueueItemStatus): { label: string; classes: string } {
  switch (status) {
    case "ok":
      return { label: "ok", classes: "bg-emerald-100 text-emerald-700" };
    case "error":
      return { label: "error", classes: "bg-red-100 text-red-700" };
    case "procesando":
      return { label: "…", classes: "bg-amber-100 text-amber-700" };
    default:
      return { label: "—", classes: "bg-neutral-100 text-neutral-500" };
  }
}

export function FileQueue() {
  const queue = useWehiStore((s) => s.queue);
  const selectedPath = useWehiStore((s) => s.selectedPath);
  const selectPath = useWehiStore((s) => s.selectPath);

  const parentRef = useRef<HTMLDivElement | null>(null);

  const virtualizer = useVirtualizer({
    count: queue.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 32,
    overscan: 12,
  });

  if (queue.length === 0) {
    return (
      <div className="border border-neutral-200 rounded-lg p-4 bg-white text-sm text-neutral-500">
        Aún no hay archivos. Elige una carpeta para empezar.
      </div>
    );
  }

  return (
    <div className="border border-neutral-200 rounded-lg bg-white flex flex-col flex-1 min-h-0">
      <div className="px-3 py-2 border-b border-neutral-100 text-xs text-neutral-600 font-medium">
        Cola — {queue.length.toLocaleString()} archivos
      </div>
      <div ref={parentRef} className="flex-1 overflow-auto">
        <div
          style={{
            height: `${virtualizer.getTotalSize()}px`,
            width: "100%",
            position: "relative",
          }}
        >
          {virtualizer.getVirtualItems().map((row) => {
            const item = queue[row.index];
            const badge = statusBadge(item.status);
            const selected = item.path === selectedPath;
            const isVideo = isVideoPath(item.path);
            return (
              <button
                type="button"
                key={item.path}
                onClick={() => selectPath(item.path)}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  height: `${row.size}px`,
                  transform: `translateY(${row.start}px)`,
                }}
                className={`flex items-center gap-2 px-3 text-left text-xs hover:bg-neutral-50 ${
                  selected ? "bg-neutral-100 font-medium" : ""
                }`}
              >
                <span
                  className={`inline-block w-12 text-[10px] px-1.5 py-0.5 rounded text-center ${badge.classes}`}
                >
                  {badge.label}
                </span>
                {isVideo && (
                  <span
                    className="inline-block text-[9px] px-1 py-0.5 rounded bg-violet-100 text-violet-700"
                    title="Archivo de video"
                  >
                    VID
                  </span>
                )}
                <span className="truncate flex-1 text-neutral-700">
                  {item.name}
                </span>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
