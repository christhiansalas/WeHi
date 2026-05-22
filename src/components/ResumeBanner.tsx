import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { useWehiStore } from "../store";
import type { ActiveBatch, QueueItem } from "../types";

function fileName(p: string): string {
  const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
  return i >= 0 ? p.slice(i + 1) : p;
}

export function ResumeBanner() {
  const [batch, setBatch] = useState<ActiveBatch | null>(null);
  const [busy, setBusy] = useState(false);
  const setPreset = useWehiStore((s) => s.setPreset);
  const setBatchFiles = useWehiStore((s) => s.setBatchFiles);
  const startBatch = useWehiStore((s) => s.startBatch);
  const setStage = useWehiStore((s) => s.setStage);

  useEffect(() => {
    invoke<ActiveBatch | null>("get_active_batch")
      .then((b) => {
        if (b && b.files.length > b.completed.length + b.failed.length) {
          setBatch(b);
        }
      })
      .catch(() => {
        /* sin lote pendiente */
      });
  }, []);

  if (!batch) return null;

  const done = new Set([...batch.completed, ...batch.failed]);
  const pending = batch.files.filter((p) => !done.has(p));

  async function resume() {
    if (!batch) return;
    setBusy(true);
    try {
      setPreset(batch.preset);
      const items: QueueItem[] = pending.map((p) => ({
        path: p,
        name: fileName(p),
        status: "pendiente",
      }));
      setBatchFiles(items);
      startBatch(pending.length);
      await invoke("process_batch", {
        files: pending,
        preset: batch.preset,
      });
      setStage("procesar");
      setBatch(null);
    } catch (e) {
      console.error(e);
    } finally {
      setBusy(false);
    }
  }

  async function discard() {
    setBusy(true);
    try {
      await invoke("clear_active_batch");
      setBatch(null);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mx-4 mt-3 px-3 py-2 rounded-lg border border-amber-300 bg-amber-50 text-[11px] flex items-center gap-3 flex-wrap">
      <div className="flex-1">
        <div className="font-medium text-amber-900">
          Procesamiento sin terminar detectado
        </div>
        <div className="text-amber-800">
          {pending.length} archivo(s) pendiente(s) de{" "}
          {batch.files.length}. Preset: <span className="font-mono">{batch.preset.name}</span>.
        </div>
      </div>
      <button
        type="button"
        onClick={resume}
        disabled={busy}
        className="text-[11px] px-3 py-1 bg-neutral-900 text-white rounded hover:bg-neutral-700 disabled:opacity-50"
      >
        Continuar
      </button>
      <button
        type="button"
        onClick={discard}
        disabled={busy}
        className="text-[11px] px-3 py-1 bg-white border border-neutral-300 rounded hover:bg-neutral-50 disabled:opacity-50"
      >
        Descartar
      </button>
    </div>
  );
}
