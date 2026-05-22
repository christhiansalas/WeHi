import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { useWehiStore } from "../store";
import type {
  FileAppearedEvent,
  QueueItem,
  WatchStatus,
} from "../types";

function fileName(p: string): string {
  const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
  return i >= 0 ? p.slice(i + 1) : p;
}

/** Modo tethered shoot: observa la carpeta source y añade los
 *  archivos nuevos a la cola a medida que aparecen. Opcionalmente
 *  los procesa de inmediato con el preset activo. */
export function TetheredControl() {
  const sourceFolder = useWehiStore((s) => s.sourceFolder);
  const queue = useWehiStore((s) => s.queue);
  const [status, setStatus] = useState<WatchStatus>({
    watching: false,
    folder: null,
  });
  const [autoProcess, setAutoProcess] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [newCount, setNewCount] = useState(0);

  // Estado inicial del backend (puede haber un watcher de una sesión
  // anterior si la app no se cerró limpia... improbable, pero por si).
  useEffect(() => {
    invoke<WatchStatus>("watch_status")
      .then(setStatus)
      .catch(() => {
        /* noop */
      });
  }, []);

  // Suscripción al evento de archivo nuevo.
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    (async () => {
      unlisten = await listen<FileAppearedEvent>(
        "file_appeared",
        async (event) => {
          const path = event.payload.path;
          const store = useWehiStore.getState();

          // Evita duplicar si el archivo ya estaba en la cola.
          if (store.queue.some((q) => q.path === path)) return;

          const item: QueueItem = {
            path,
            name: fileName(path),
            status: "pendiente",
          };
          useWehiStore.setState({
            queue: [...store.queue, item],
            selectedPath: store.selectedPath ?? path,
          });
          setNewCount((n) => n + 1);

          if (autoProcess) {
            try {
              await invoke("process_batch", {
                files: [path],
                preset: store.preset,
              });
            } catch (e) {
              setError(String(e));
            }
          }
        },
      );
    })();
    return () => {
      unlisten?.();
    };
  }, [autoProcess]);

  async function toggle() {
    setError(null);
    setBusy(true);
    try {
      if (status.watching) {
        await invoke("stop_watching");
        setStatus({ watching: false, folder: null });
      } else {
        if (!sourceFolder) {
          setError("Elige una carpeta antes de iniciar tethered shoot.");
          return;
        }
        await invoke("start_watching", { path: sourceFolder });
        setStatus({ watching: true, folder: sourceFolder });
        setNewCount(0);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="border border-neutral-200 rounded-lg p-3 bg-white space-y-2">
      <div className="flex items-center justify-between">
        <div className="text-xs font-medium text-neutral-700">
          Tethered shoot
        </div>
        <button
          type="button"
          onClick={toggle}
          disabled={busy || (!status.watching && !sourceFolder)}
          className={`text-[11px] px-3 py-1 rounded ${
            status.watching
              ? "bg-red-600 text-white hover:bg-red-700"
              : "bg-neutral-900 text-white hover:bg-neutral-700"
          } disabled:opacity-50`}
        >
          {status.watching ? "Detener" : "Iniciar"}
        </button>
      </div>
      {status.watching && status.folder && (
        <div className="text-[10px] text-emerald-700 break-all">
          ● Observando: {status.folder}
        </div>
      )}
      {!status.watching && !sourceFolder && (
        <div className="text-[10px] text-neutral-500 italic">
          Elige una carpeta primero.
        </div>
      )}
      <label className="text-[11px] text-neutral-700 flex items-center gap-2">
        <input
          type="checkbox"
          checked={autoProcess}
          onChange={(e) => setAutoProcess(e.target.checked)}
        />
        Auto-procesar nuevos archivos con el preset activo
      </label>
      {status.watching && (
        <div className="text-[10px] text-neutral-500">
          {newCount} archivo(s) detectado(s) desde el inicio
          {queue.length > 0 && (
            <span className="ml-1">· {queue.length} en cola</span>
          )}
        </div>
      )}
      {error && (
        <div className="text-[10px] text-red-600 break-all">{error}</div>
      )}
    </div>
  );
}
