import { useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";

import { useWehiStore } from "../store";
import type { QueueItemStatus } from "../types";

function statusBadge(status: QueueItemStatus): {
  label: string;
  classes: string;
} {
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

function fileName(p: string): string {
  const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
  return i >= 0 ? p.slice(i + 1) : p;
}

function formatUs(us: number): string {
  const s = Math.floor(us / 1_000_000);
  const m = Math.floor(s / 60);
  const sec = s % 60;
  return `${m}:${sec.toString().padStart(2, "0")}`;
}

export function ProgressPanel() {
  const batchFiles = useWehiStore((s) => s.batchFiles);
  const procesadas = useWehiStore((s) => s.procesadas);
  const total = useWehiStore((s) => s.total);
  const archivoActual = useWehiStore((s) => s.archivoActual);
  const batchState = useWehiStore((s) => s.batchState);
  const errores = useWehiStore((s) => s.errores);
  const videoProgress = useWehiStore((s) => s.videoProgress);
  const resetBatch = useWehiStore((s) => s.resetBatch);
  const setBatchFiles = useWehiStore((s) => s.setBatchFiles);

  const [errorsOpen, setErrorsOpen] = useState(false);

  const exitosas = useMemo(
    () => batchFiles.filter((f) => f.status === "ok").length,
    [batchFiles],
  );
  const fallidas = useMemo(
    () => batchFiles.filter((f) => f.status === "error").length,
    [batchFiles],
  );

  const pct =
    total > 0 ? Math.min(100, Math.round((procesadas / total) * 100)) : 0;

  const parentRef = useRef<HTMLDivElement | null>(null);
  const virtualizer = useVirtualizer({
    count: batchFiles.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 28,
    overscan: 12,
  });

  function closeSummary() {
    resetBatch();
    setBatchFiles([]);
  }

  return (
    <div className="h-full flex flex-col">
      <div className="px-3 py-2 border-b border-neutral-200 bg-white">
        <div className="flex items-center justify-between mb-1">
          <div className="text-xs font-medium text-neutral-800">
            {batchState === "running"
              ? "Procesando lote…"
              : batchState === "cancelled"
                ? "Lote cancelado"
                : batchState === "done"
                  ? "Lote terminado"
                  : "Lote"}
          </div>
          <div className="text-[11px] text-neutral-500">
            {procesadas.toLocaleString()} / {total.toLocaleString()}
            <span className="ml-2 text-neutral-700">{pct}%</span>
          </div>
        </div>
        <div className="h-1.5 bg-neutral-200 rounded overflow-hidden">
          <div
            className={`h-full transition-[width] ${
              batchState === "running" ? "bg-neutral-900" : "bg-emerald-500"
            }`}
            style={{ width: `${pct}%` }}
          />
        </div>
        {batchState === "running" && archivoActual && (
          <div className="mt-1 text-[10px] text-neutral-500 truncate">
            {archivoActual}
          </div>
        )}
        {batchState === "running" && videoProgress && videoProgress.totalUs > 0 && (
          <div className="mt-1 space-y-0.5">
            <div className="text-[10px] text-neutral-600 flex items-center justify-between">
              <span className="truncate">
                Video: {fileName(videoProgress.ruta)}
              </span>
              <span className="ml-2 shrink-0">
                {formatUs(videoProgress.currentUs)} /{" "}
                {formatUs(videoProgress.totalUs)} ·{" "}
                {Math.round((videoProgress.currentUs / videoProgress.totalUs) * 100)}%
              </span>
            </div>
            <div className="h-1 bg-neutral-200 rounded overflow-hidden">
              <div
                className="h-full bg-violet-500 transition-[width]"
                style={{
                  width: `${Math.min(100, (videoProgress.currentUs / videoProgress.totalUs) * 100)}%`,
                }}
              />
            </div>
          </div>
        )}
        {batchState !== "running" && batchState !== "idle" && (
          <div className="mt-2 flex items-center gap-3 text-[11px]">
            <span className="text-emerald-700">✓ {exitosas} exitosas</span>
            <span className="text-red-700">✗ {fallidas} fallidas</span>
            {errores.length > 0 && (
              <button
                type="button"
                onClick={() => setErrorsOpen((v) => !v)}
                className="text-neutral-600 underline"
              >
                {errorsOpen ? "Ocultar errores" : `Ver errores (${errores.length})`}
              </button>
            )}
            <button
              type="button"
              onClick={closeSummary}
              className="ml-auto text-xs px-2 py-0.5 bg-white border border-neutral-300 rounded hover:bg-neutral-100"
            >
              Cerrar
            </button>
          </div>
        )}
        {errorsOpen && errores.length > 0 && (
          <div className="mt-2 max-h-32 overflow-auto border border-neutral-200 rounded bg-neutral-50 p-2 space-y-1">
            {errores.map((e, i) => (
              <div key={`${e.ruta}-${i}`} className="text-[10px]">
                <div className="text-neutral-800 truncate">{e.ruta}</div>
                <div className="text-red-600 break-all">{e.mensaje}</div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div ref={parentRef} className="flex-1 overflow-auto bg-white">
        {batchFiles.length === 0 ? (
          <div className="h-full flex items-center justify-center text-xs text-neutral-400">
            Sin archivos en el lote.
          </div>
        ) : (
          <div
            style={{
              height: `${virtualizer.getTotalSize()}px`,
              width: "100%",
              position: "relative",
            }}
          >
            {virtualizer.getVirtualItems().map((row) => {
              const item = batchFiles[row.index];
              const badge = statusBadge(item.status);
              return (
                <div
                  key={item.path}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: "100%",
                    height: `${row.size}px`,
                    transform: `translateY(${row.start}px)`,
                  }}
                  className="flex items-center gap-2 px-3 text-[11px] border-b border-neutral-50"
                >
                  <span
                    className={`inline-block w-12 text-[10px] px-1.5 py-0.5 rounded text-center ${badge.classes}`}
                  >
                    {badge.label}
                  </span>
                  <span className="flex-1 truncate text-neutral-700">
                    {item.name}
                  </span>
                  {item.error && (
                    <span
                      className="text-red-600 truncate max-w-[50%]"
                      title={item.error}
                    >
                      {item.error}
                    </span>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
