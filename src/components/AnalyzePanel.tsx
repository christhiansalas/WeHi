import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useVirtualizer } from "@tanstack/react-virtual";

import type {
  AnalysisDoneEvent,
  AnalysisProgressEvent,
  AnalysisRecord,
  CullStatus,
  MoveRejectsReport,
  ScoringWeights,
} from "../types";
import { useWehiStore } from "../store";

function clamp(v: number, lo: number, hi: number): number {
  return Math.max(lo, Math.min(hi, v));
}

/** Recalcula el composite_score en el frontend a partir de los
 *  sub_scores y los pesos actuales. La fórmula refleja
 *  `imganalyze::scoring::score_batch`. */
function recomposite(rec: AnalysisRecord, w: ScoringWeights): number {
  const { sharpness, exposure, aesthetic, resolution } = rec.sub_scores;
  let bonus = 0;
  if (rec.faces && rec.faces.count > 0) {
    const sizeNorm = clamp(rec.faces.largest_face_frac / 0.2, 0, 1);
    const centered = clamp(rec.faces.main_face_centered, 0, 1);
    bonus = clamp(
      w.face_bonus_max * (0.5 * centered + 0.5 * sizeNorm),
      0,
      w.face_bonus_max,
    );
  }
  return clamp(
    w.w_sharpness * sharpness +
      w.w_exposure * exposure +
      w.w_aesthetic * aesthetic +
      w.w_resolution * resolution +
      bonus,
    0,
    1,
  );
}

function Bar({
  label,
  value,
  tint,
}: {
  label: string;
  value: number;
  tint: string;
}) {
  const pct = `${Math.round(clamp(value, 0, 1) * 100)}%`;
  return (
    <div className="flex items-center gap-1 text-[10px]">
      <span className="w-12 text-neutral-500 shrink-0">{label}</span>
      <div className="h-1.5 flex-1 bg-neutral-200 rounded overflow-hidden">
        <div className={`h-full ${tint}`} style={{ width: pct }} />
      </div>
      <span className="w-7 text-right text-neutral-600 shrink-0">{pct}</span>
    </div>
  );
}

function StatusButtons({
  status,
  onChange,
}: {
  status: CullStatus;
  onChange: (next: CullStatus) => void;
}) {
  const btn = (label: string, value: CullStatus, classes: string) => (
    <button
      type="button"
      onClick={() => onChange(value)}
      className={`text-[10px] px-2 py-0.5 rounded border ${
        status === value
          ? classes
          : "bg-white text-neutral-600 border-neutral-300 hover:bg-neutral-50"
      }`}
    >
      {label}
    </button>
  );
  return (
    <div className="flex gap-1">
      {btn("Keep", "Keep", "bg-emerald-600 text-white border-emerald-600")}
      {btn("?", "Undecided", "bg-neutral-700 text-white border-neutral-700")}
      {btn("Reject", "Reject", "bg-red-600 text-white border-red-600")}
    </div>
  );
}

function RowThumb({ path }: { path: string }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    invoke<string>("get_thumbnail_data_url", { file: path, size: 96 })
      .then((url) => {
        if (!cancelled) setSrc(url);
      })
      .catch(() => {
        if (!cancelled) setSrc(null);
      });
    return () => {
      cancelled = true;
    };
  }, [path]);
  return (
    <div className="w-16 h-16 bg-neutral-200 rounded overflow-hidden shrink-0 flex items-center justify-center">
      {src ? (
        <img src={src} alt="" className="w-full h-full object-cover" />
      ) : (
        <span className="text-[9px] text-neutral-400">…</span>
      )}
    </div>
  );
}

function fileName(p: string): string {
  const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
  return i >= 0 ? p.slice(i + 1) : p;
}

export function AnalyzePanel() {
  const queue = useWehiStore((s) => s.queue);
  const records = useWehiStore((s) => s.analysisRecords);
  const setRecords = useWehiStore((s) => s.setAnalysisRecords);
  const running = useWehiStore((s) => s.analysisRunning);
  const setRunning = useWehiStore((s) => s.setAnalysisRunning);
  const progress = useWehiStore((s) => s.analysisProgress);
  const setProgress = useWehiStore((s) => s.setAnalysisProgress);
  const setLocalCullStatus = useWehiStore((s) => s.setLocalCullStatus);
  const weights = useWehiStore((s) => s.weights);
  const patchWeights = useWehiStore((s) => s.patchWeights);
  const setWeights = useWehiStore((s) => s.setWeights);
  const dedupConfig = useWehiStore((s) => s.dedupConfig);
  const patchDedupConfig = useWehiStore((s) => s.patchDedupConfig);
  const setDedupConfig = useWehiStore((s) => s.setDedupConfig);

  const [useAi, setUseAi] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [moveReport, setMoveReport] = useState<MoveRejectsReport | null>(null);
  const [moveErrorsOpen, setMoveErrorsOpen] = useState(false);

  // Suscripción a eventos de progreso.
  useEffect(() => {
    let unlistenProgress: UnlistenFn | null = null;
    let unlistenDone: UnlistenFn | null = null;
    (async () => {
      unlistenProgress = await listen<AnalysisProgressEvent>(
        "analysis_progress",
        (event) => {
          setProgress({
            analizadas: event.payload.analizadas,
            total: event.payload.total,
            archivo: event.payload.archivo_actual,
          });
        },
      );
      unlistenDone = await listen<AnalysisDoneEvent>(
        "analysis_done",
        async () => {
          try {
            const results = await invoke<AnalysisRecord[]>(
              "get_analysis_results",
            );
            setRecords(results);
          } catch (e) {
            setError(String(e));
          } finally {
            setRunning(false);
          }
        },
      );
    })();
    return () => {
      unlistenProgress?.();
      unlistenDone?.();
    };
  }, [setProgress, setRecords, setRunning]);

  async function startAnalyze() {
    setError(null);
    setRecords([]);
    setProgress({ analizadas: 0, total: queue.length, archivo: null });
    setRunning(true);
    try {
      await invoke("analyze_batch", {
        files: queue.map((q) => q.path),
        useAi,
        dedupConfig,
      });
    } catch (e) {
      setError(String(e));
      setRunning(false);
    }
  }

  async function cancelAnalyze() {
    try {
      await invoke("cancel_analysis");
    } catch (e) {
      setError(String(e));
    }
  }

  async function changeStatus(path: string, status: CullStatus) {
    setLocalCullStatus(path, status);
    try {
      await invoke("set_cull_status", { path, status });
    } catch (e) {
      setError(String(e));
    }
  }

  async function moveRejects() {
    const ok = window.confirm(
      "Los archivos marcados como Reject serán MOVIDOS a una subcarpeta (no se eliminan). ¿Continuar?",
    );
    if (!ok) return;
    setError(null);
    setMoveReport(null);
    setMoveErrorsOpen(false);
    try {
      const report = await invoke<MoveRejectsReport>("move_rejects", {
        targetSubfolder: "descartes",
      });
      setMoveReport(report);
      setMoveErrorsOpen(report.errores.length > 0);
      // Refresca los registros desde el backend para que la UI no
      // muestre como Reject archivos que ya no existen en su ruta.
      const fresh = await invoke<AnalysisRecord[]>("get_analysis_results");
      useWehiStore.getState().setAnalysisRecords(fresh);
    } catch (e) {
      setError(String(e));
    }
  }

  // Re-rank en vivo en función de los pesos actuales. Agrupa por
  // duplicate_group conservando el orden por composite recalculado.
  const ranked = useMemo(() => {
    const withScore = records.map((r) => ({
      record: r,
      score: recomposite(r, weights),
    }));
    withScore.sort((a, b) => b.score - a.score);
    return withScore;
  }, [records, weights]);

  const parentRef = useRef<HTMLDivElement | null>(null);
  const virtualizer = useVirtualizer({
    count: ranked.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 96,
    overscan: 6,
  });

  const totalRejects = useMemo(
    () => records.filter((r) => r.status === "Reject").length,
    [records],
  );

  return (
    <div className="h-full flex flex-col">
      <div className="px-3 py-2 border-b border-neutral-200 bg-white flex items-center gap-3 flex-wrap">
        <button
          type="button"
          onClick={startAnalyze}
          disabled={running || queue.length === 0}
          className="text-xs px-3 py-1.5 bg-neutral-900 text-white rounded hover:bg-neutral-700 disabled:opacity-50"
        >
          {running ? "Analizando…" : "Analizar"}
        </button>
        {running && (
          <button
            type="button"
            onClick={cancelAnalyze}
            className="text-xs px-3 py-1.5 bg-white border border-neutral-300 rounded hover:bg-neutral-100"
          >
            Cancelar
          </button>
        )}
        <label className="text-[11px] text-neutral-700 flex items-center gap-1.5">
          <input
            type="checkbox"
            checked={useAi}
            onChange={(e) => setUseAi(e.target.checked)}
            disabled={running}
          />
          Incluir análisis de IA (estética y caras)
        </label>
        {running && (
          <div className="text-[11px] text-neutral-500">
            {progress.analizadas.toLocaleString()} /{" "}
            {progress.total.toLocaleString()}
            {progress.archivo && (
              <span className="ml-2 text-neutral-400 truncate inline-block max-w-[200px] align-bottom">
                {fileName(progress.archivo)}
              </span>
            )}
          </div>
        )}
        <div className="ml-auto flex items-center gap-2">
          <span className="text-[11px] text-neutral-500">
            Reject: {totalRejects}
          </span>
          <button
            type="button"
            onClick={moveRejects}
            disabled={totalRejects === 0 || running}
            className="text-xs px-2 py-1.5 bg-red-600 text-white rounded hover:bg-red-700 disabled:opacity-50"
          >
            Mover descartes…
          </button>
        </div>
      </div>

      {/* Panel de pesos */}
      <div className="px-3 py-2 border-b border-neutral-100 bg-neutral-50 grid grid-cols-5 gap-2">
        {(
          [
            ["w_sharpness", "Nitidez"],
            ["w_exposure", "Exposición"],
            ["w_aesthetic", "Estética"],
            ["w_resolution", "Resolución"],
            ["face_bonus_max", "Bono caras"],
          ] as const
        ).map(([key, label]) => (
          <label key={key} className="text-[10px] text-neutral-600 block">
            <div className="flex items-center justify-between mb-0.5">
              <span>{label}</span>
              <span className="text-neutral-500">{weights[key].toFixed(2)}</span>
            </div>
            <input
              type="range"
              min={0}
              max={1}
              step={0.01}
              value={weights[key]}
              onChange={(e) =>
                patchWeights({ [key]: Number(e.target.value) } as Partial<ScoringWeights>)
              }
              className="w-full"
            />
          </label>
        ))}
        <div className="col-span-5 flex justify-end">
          <button
            type="button"
            onClick={() =>
              setWeights({
                w_sharpness: 0.3,
                w_exposure: 0.22,
                w_aesthetic: 0.38,
                w_resolution: 0.1,
                face_bonus_max: 0.1,
              })
            }
            className="text-[10px] px-2 py-0.5 text-neutral-600 hover:text-neutral-900"
          >
            Restablecer pesos
          </button>
        </div>
      </div>

      {/* Panel de dedup (umbrales editables). Aplica en el próximo análisis. */}
      <div className="px-3 py-2 border-b border-neutral-100 bg-neutral-50 grid grid-cols-2 gap-3">
        <label className="text-[10px] text-neutral-600 block">
          <div className="flex items-center justify-between mb-0.5">
            <span>
              Umbral de similitud (Hamming){" "}
              <span className="text-neutral-400">menos = más estricto</span>
            </span>
            <span className="text-neutral-500">{dedupConfig.hamming_threshold}</span>
          </div>
          <input
            type="range"
            min={0}
            max={30}
            step={1}
            value={dedupConfig.hamming_threshold}
            onChange={(e) =>
              patchDedupConfig({ hamming_threshold: Number(e.target.value) })
            }
            className="w-full"
            disabled={running}
          />
        </label>
        <label className="text-[10px] text-neutral-600 block">
          <div className="flex items-center justify-between mb-0.5">
            <span>
              Ventana temporal (s){" "}
              <span className="text-neutral-400">0 = solo Hamming</span>
            </span>
            <span className="text-neutral-500">{dedupConfig.time_window_seconds}</span>
          </div>
          <input
            type="range"
            min={0}
            max={600}
            step={5}
            value={dedupConfig.time_window_seconds}
            onChange={(e) =>
              patchDedupConfig({ time_window_seconds: Number(e.target.value) })
            }
            className="w-full"
            disabled={running}
          />
        </label>
        <div className="col-span-2 flex justify-end">
          <button
            type="button"
            onClick={() =>
              setDedupConfig({ hamming_threshold: 10, time_window_seconds: 30 })
            }
            className="text-[10px] px-2 py-0.5 text-neutral-600 hover:text-neutral-900"
            disabled={running}
          >
            Restablecer dedup
          </button>
        </div>
      </div>

      {error && (
        <div className="px-3 py-1 text-[11px] bg-white text-red-600 break-all">
          {error}
        </div>
      )}
      {moveReport && (
        <div className="px-3 py-2 text-[11px] bg-white border-b border-neutral-100 space-y-1">
          <div className="flex items-center gap-3 flex-wrap">
            <span className="text-neutral-700">
              Considerados (Reject): {moveReport.considerados}
            </span>
            <span className="text-emerald-700">
              ✓ Movidos: {moveReport.movidos.length}
            </span>
            <span className={moveReport.errores.length > 0 ? "text-red-700" : "text-neutral-500"}>
              ✗ Errores: {moveReport.errores.length}
            </span>
            {moveReport.errores.length > 0 && (
              <button
                type="button"
                onClick={() => setMoveErrorsOpen((v) => !v)}
                className="text-neutral-600 underline"
              >
                {moveErrorsOpen ? "Ocultar detalles" : "Ver detalles"}
              </button>
            )}
            <button
              type="button"
              onClick={() => setMoveReport(null)}
              className="ml-auto text-neutral-500 hover:text-neutral-800"
            >
              ×
            </button>
          </div>
          {moveReport.considerados === 0 && (
            <div className="text-neutral-500 italic">
              No había registros con estado "Reject" en el lote. Marca al menos uno y reintenta.
            </div>
          )}
          {moveErrorsOpen && moveReport.errores.length > 0 && (
            <div className="max-h-40 overflow-auto border border-neutral-200 rounded bg-neutral-50 p-2 space-y-1">
              {moveReport.errores.map((e, i) => (
                <div key={`${e.ruta}-${i}`} className="text-[10px]">
                  <div className="text-neutral-800 truncate">{e.ruta}</div>
                  <div className="text-red-600 break-all">{e.mensaje}</div>
                </div>
              ))}
              <div className="mt-2 text-[10px] text-neutral-500 italic">
                Si ves "Operation not permitted": macOS bloquea apps sin firma de Developer ID al
                tocar carpetas protegidas. Concede a WeHi acceso en
                <span className="font-medium">
                  {" "}Ajustes del sistema → Privacidad y seguridad → Archivos y carpetas
                </span>
                {" "}o{" "}
                <span className="font-medium">Acceso completo al disco</span>.
              </div>
            </div>
          )}
        </div>
      )}

      {/* Lista virtualizada */}
      <div ref={parentRef} className="flex-1 overflow-auto bg-white">
        {ranked.length === 0 ? (
          <div className="h-full flex items-center justify-center text-xs text-neutral-400 p-6 text-center">
            {running
              ? "Analizando lote…"
              : "Aún no hay resultados. Pulsa Analizar."}
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
              const { record, score } = ranked[row.index];
              const prev = row.index > 0 ? ranked[row.index - 1].record : null;
              const groupStart =
                record.duplicate_group !== null &&
                prev?.duplicate_group !== record.duplicate_group;
              return (
                <div
                  key={record.path}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: "100%",
                    height: `${row.size}px`,
                    transform: `translateY(${row.start}px)`,
                  }}
                  className={`flex items-center gap-3 px-3 border-b border-neutral-100 ${
                    record.status === "Reject" ? "bg-red-50" : ""
                  } ${record.is_group_winner ? "bg-emerald-50" : ""}`}
                >
                  <RowThumb path={record.path} />
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <div className="text-xs font-medium text-neutral-800 truncate">
                        {fileName(record.path)}
                      </div>
                      {record.duplicate_group !== null && (
                        <span
                          className={`text-[9px] px-1 py-0.5 rounded ${
                            record.is_group_winner
                              ? "bg-emerald-600 text-white"
                              : "bg-neutral-200 text-neutral-700"
                          }`}
                          title="Grupo de duplicados"
                        >
                          {record.is_group_winner ? "★" : "≈"} g
                          {record.duplicate_group}
                          {groupStart ? " (1)" : ""}
                        </span>
                      )}
                    </div>
                    <div className="text-[10px] text-neutral-500">
                      {record.width}×{record.height} ·
                      composite {score.toFixed(3)}
                    </div>
                    <div className="mt-1 grid grid-cols-2 gap-x-3 gap-y-0.5">
                      <Bar
                        label="nitidez"
                        value={record.sub_scores.sharpness}
                        tint="bg-sky-500"
                      />
                      <Bar
                        label="expos."
                        value={record.sub_scores.exposure}
                        tint="bg-amber-500"
                      />
                      <Bar
                        label="estét."
                        value={record.sub_scores.aesthetic}
                        tint="bg-violet-500"
                      />
                      <Bar
                        label="caras"
                        value={record.sub_scores.face_bonus}
                        tint="bg-emerald-500"
                      />
                    </div>
                  </div>
                  <StatusButtons
                    status={record.status}
                    onChange={(s) => void changeStatus(record.path, s)}
                  />
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
