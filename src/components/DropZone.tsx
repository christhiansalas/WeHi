import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { UnlistenFn } from "@tauri-apps/api/event";

import { useWehiStore } from "../store";
import { isVideoPath, type FfmpegStatus } from "../types";

export function DropZone() {
  const sourceFolder = useWehiStore((s) => s.sourceFolder);
  const queue = useWehiStore((s) => s.queue);
  const setSourceFolder = useWehiStore((s) => s.setSourceFolder);
  const setQueueFromPaths = useWehiStore((s) => s.setQueueFromPaths);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [ffmpeg, setFfmpeg] = useState<FfmpegStatus | null>(null);

  const [hovering, setHovering] = useState(false);

  useEffect(() => {
    invoke<FfmpegStatus>("check_ffmpeg")
      .then(setFfmpeg)
      .catch(() => setFfmpeg({ available: false, path: null }));
  }, []);

  // Drag-and-drop sobre la ventana (nativo de Tauri 2).
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    (async () => {
      const win = getCurrentWebviewWindow();
      unlisten = await win.onDragDropEvent(async (event) => {
        const payload = event.payload;
        if (payload.type === "over") {
          setHovering(true);
        } else if (payload.type === "leave") {
          setHovering(false);
        } else if (payload.type === "drop") {
          setHovering(false);
          if (payload.paths && payload.paths.length > 0) {
            setLoading(true);
            setError(null);
            try {
              const files = await invoke<string[]>("scan_paths", {
                paths: payload.paths,
              });
              if (files.length === 0) {
                setError(
                  "Ningún archivo soportado en lo arrastrado.",
                );
                return;
              }
              // Carpeta como fuente: si la primera ruta es directorio, úsala.
              const firstPath = payload.paths[0];
              setSourceFolder(firstPath);
              setQueueFromPaths(files);
            } catch (e) {
              setError(String(e));
            } finally {
              setLoading(false);
            }
          }
        }
      });
    })();
    return () => {
      unlisten?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const videoCount = queue.filter((q) => isVideoPath(q.path)).length;

  async function pickFolder() {
    setError(null);
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected !== "string") return;
      setLoading(true);
      const files = await invoke<string[]>("scan_folder", { path: selected });
      setSourceFolder(selected);
      setQueueFromPaths(files);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div
      className={`border rounded-lg p-4 bg-white transition ${
        hovering
          ? "border-neutral-900 ring-2 ring-neutral-900/30 bg-neutral-50"
          : "border-neutral-200"
      }`}
    >
      <button
        type="button"
        onClick={pickFolder}
        disabled={loading}
        className="w-full px-4 py-2 bg-neutral-900 text-white rounded-md hover:bg-neutral-700 transition disabled:opacity-50"
      >
        {loading ? "Escaneando…" : "Elegir carpeta"}
      </button>
      <div className="mt-2 text-[10px] text-neutral-400 text-center">
        o arrastra carpetas/archivos aquí
      </div>
      {sourceFolder && (
        <div className="mt-3 text-xs text-neutral-500 break-all">
          <div className="font-medium text-neutral-700">Carpeta:</div>
          <div>{sourceFolder}</div>
          <div className="mt-2 text-neutral-700">
            {queue.length.toLocaleString()} archivo(s) detectado(s)
            {videoCount > 0 && (
              <span className="ml-1 text-violet-700">
                ({videoCount.toLocaleString()} video{videoCount === 1 ? "" : "s"})
              </span>
            )}
          </div>
        </div>
      )}
      {ffmpeg && !ffmpeg.available && videoCount > 0 && (
        <div className="mt-3 text-[11px] text-amber-700 bg-amber-50 border border-amber-200 rounded p-2">
          <div className="font-medium">ffmpeg no detectado</div>
          <div className="mt-0.5">
            Hay {videoCount} video(s) en la cola, pero no podrán procesarse sin
            ffmpeg. Instala con: <code className="bg-amber-100 px-1">brew install ffmpeg</code>{" "}
            (macOS), <code className="bg-amber-100 px-1">winget install ffmpeg</code> (Windows)
            o <code className="bg-amber-100 px-1">apt install ffmpeg</code> (Linux).
          </div>
        </div>
      )}
      {error && (
        <div className="mt-3 text-xs text-red-600 break-all">{error}</div>
      )}
    </div>
  );
}
