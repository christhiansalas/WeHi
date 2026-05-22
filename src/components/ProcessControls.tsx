import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { useWehiStore } from "../store";
import type { QueueItem } from "../types";

function fileName(p: string): string {
  const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
  return i >= 0 ? p.slice(i + 1) : p;
}

export function ProcessControls() {
  const queue = useWehiStore((s) => s.queue);
  const analysis = useWehiStore((s) => s.analysisRecords);
  const preset = useWehiStore((s) => s.preset);
  const batchState = useWehiStore((s) => s.batchState);
  const setBatchFiles = useWehiStore((s) => s.setBatchFiles);
  const startBatch = useWehiStore((s) => s.startBatch);

  const [includeUndecided, setIncludeUndecided] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const hasAnalysis = analysis.length > 0;
  const running = batchState === "running";
  const noFolder = queue.length === 0;
  const noFolderOutput = !preset.output.folder;

  function selectFiles(): string[] {
    if (!hasAnalysis) return queue.map((q) => q.path);
    const allow = new Set<string>();
    for (const r of analysis) {
      if (r.status === "Keep") allow.add(r.path);
      else if (includeUndecided && r.status === "Undecided") allow.add(r.path);
    }
    return queue.map((q) => q.path).filter((p) => allow.has(p));
  }

  async function startProcess() {
    setError(null);
    const files = selectFiles();
    if (files.length === 0) {
      setError("No hay archivos para procesar con el filtro actual.");
      return;
    }
    const batchItems: QueueItem[] = files.map((p) => ({
      path: p,
      name: fileName(p),
      status: "pendiente",
    }));
    setBatchFiles(batchItems);
    startBatch(files.length);
    try {
      await invoke("process_batch", { files, preset });
    } catch (e) {
      setError(String(e));
    }
  }

  async function cancelProcess() {
    try {
      await invoke("cancel_batch");
    } catch (e) {
      setError(String(e));
    }
  }

  const filesCount = selectFiles().length;

  return (
    <div className="border border-neutral-200 rounded-lg p-3 bg-white space-y-2">
      <div className="flex items-center justify-between">
        <div className="text-xs font-medium text-neutral-700">Procesar lote</div>
        <div className="text-[10px] text-neutral-500">
          {filesCount.toLocaleString()} archivo(s)
        </div>
      </div>
      {hasAnalysis && (
        <label className="flex items-center gap-2 text-[11px] text-neutral-700">
          <input
            type="checkbox"
            checked={includeUndecided}
            onChange={(e) => setIncludeUndecided(e.target.checked)}
            disabled={running}
          />
          Incluir Undecided (no solo Keep)
        </label>
      )}
      {!hasAnalysis && (
        <div className="text-[10px] text-neutral-500 italic">
          Sin análisis: procesa todos los archivos escaneados.
        </div>
      )}
      <div className="flex gap-2">
        <button
          type="button"
          onClick={startProcess}
          disabled={running || noFolder || noFolderOutput || filesCount === 0}
          className="flex-1 text-xs px-3 py-1.5 bg-neutral-900 text-white rounded hover:bg-neutral-700 disabled:opacity-50"
        >
          {running ? "Procesando…" : "Procesar lote"}
        </button>
        {running && (
          <button
            type="button"
            onClick={cancelProcess}
            className="text-xs px-3 py-1.5 bg-white border border-neutral-300 rounded hover:bg-neutral-100"
          >
            Cancelar
          </button>
        )}
      </div>
      {noFolderOutput && (
        <div className="text-[10px] text-amber-600">
          Configura la carpeta de salida en el editor de preset.
        </div>
      )}
      {error && (
        <div className="text-[10px] text-red-600 break-all">{error}</div>
      )}
    </div>
  );
}
