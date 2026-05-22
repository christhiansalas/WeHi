import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { useWehiStore } from "../store";
import type {
  BatchDoneEvent,
  FileDoneEvent,
  ProgressEvent,
  VideoProgressEvent,
} from "../types";

/** Suscribe los eventos del lote de PROCESAMIENTO al store. Debe
 *  montarse una sola vez en un componente siempre activo (App). */
export function useBatchEvents() {
  const updateProgress = useWehiStore((s) => s.updateProgress);
  const updateVideoProgress = useWehiStore((s) => s.updateVideoProgress);
  const setBatchFileStatus = useWehiStore((s) => s.setBatchFileStatus);
  const finishBatch = useWehiStore((s) => s.finishBatch);
  const setBatchErrors = useWehiStore((s) => s.setBatchErrors);

  useEffect(() => {
    let unP: UnlistenFn | null = null;
    let unV: UnlistenFn | null = null;
    let unF: UnlistenFn | null = null;
    let unD: UnlistenFn | null = null;
    (async () => {
      unP = await listen<ProgressEvent>("progress", (event) => {
        updateProgress(event.payload.procesadas, event.payload.archivo_actual);
      });
      unV = await listen<VideoProgressEvent>("video_progress", (event) => {
        updateVideoProgress(
          event.payload.ruta,
          event.payload.current_us,
          event.payload.total_us,
        );
      });
      unF = await listen<FileDoneEvent>("file_done", (event) => {
        setBatchFileStatus(
          event.payload.ruta,
          event.payload.ok ? "ok" : "error",
          event.payload.error,
        );
      });
      unD = await listen<BatchDoneEvent>("batch_done", (event) => {
        finishBatch(event.payload.cancelado);
        setBatchErrors(event.payload.errores);
      });
    })();
    return () => {
      unP?.();
      unV?.();
      unF?.();
      unD?.();
    };
  }, [
    updateProgress,
    updateVideoProgress,
    setBatchFileStatus,
    finishBatch,
    setBatchErrors,
  ]);
}
