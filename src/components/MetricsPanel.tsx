import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { Metrics } from "../types";

function Row({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="flex items-center justify-between text-[11px]">
      <span className="text-neutral-600">{label}</span>
      <span className="font-mono text-neutral-900">{value}</span>
    </div>
  );
}

export function MetricsPanel() {
  const [metrics, setMetrics] = useState<Metrics | null>(null);
  const [busy, setBusy] = useState(false);

  async function refresh() {
    try {
      const m = await invoke<Metrics>("get_metrics");
      setMetrics(m);
    } catch {
      setMetrics(null);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function toggle(enabled: boolean) {
    setBusy(true);
    try {
      await invoke("set_metrics_enabled", { enabled });
      await refresh();
    } finally {
      setBusy(false);
    }
  }

  async function clear() {
    if (!window.confirm("Borrar todas las métricas locales?")) return;
    setBusy(true);
    try {
      await invoke("clear_metrics");
      await refresh();
    } finally {
      setBusy(false);
    }
  }

  if (!metrics) {
    return (
      <div className="border border-neutral-200 rounded-lg p-3 bg-white text-xs text-neutral-400">
        Cargando métricas…
      </div>
    );
  }

  const total =
    metrics.images_processed +
    metrics.videos_processed +
    metrics.files_failed;

  return (
    <div className="border border-neutral-200 rounded-lg p-3 bg-white space-y-2">
      <div className="flex items-center justify-between">
        <div className="text-xs font-medium text-neutral-700">
          Métricas locales
        </div>
        <label className="text-[10px] text-neutral-600 flex items-center gap-1">
          <input
            type="checkbox"
            checked={metrics.enabled}
            onChange={(e) => void toggle(e.target.checked)}
            disabled={busy}
          />
          Registrar
        </label>
      </div>

      {!metrics.enabled && (
        <div className="text-[10px] text-neutral-500 italic">
          Tracking apagado. Actívalo para registrar tu uso (solo se
          guarda en este equipo).
        </div>
      )}

      <div className="space-y-0.5">
        <Row label="Imágenes procesadas" value={metrics.images_processed} />
        <Row label="Videos procesados" value={metrics.videos_processed} />
        <Row label="Archivos fallidos" value={metrics.files_failed} />
        <Row label="Lotes completados" value={metrics.batches_completed} />
        <Row label="Lotes cancelados" value={metrics.batches_cancelled} />
        <Row label="Análisis completados" value={metrics.analyses_completed} />
        <Row label="Descartes movidos" value={metrics.rejects_moved} />
        <div className="border-t border-neutral-100 pt-0.5">
          <Row label="Total de archivos" value={total} />
        </div>
      </div>

      <button
        type="button"
        onClick={clear}
        disabled={busy || total === 0}
        className="w-full text-[10px] px-2 py-1 bg-white border border-neutral-300 rounded hover:bg-neutral-50 disabled:opacity-50"
      >
        Borrar métricas
      </button>
    </div>
  );
}
