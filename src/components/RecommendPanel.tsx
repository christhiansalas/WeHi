import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { NetworkSpec, Recommendation } from "../types";
import { useWehiStore } from "../store";

function fileName(p: string): string {
  const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
  return i >= 0 ? p.slice(i + 1) : p;
}

function ThumbCrop({
  path,
  aspect,
}: {
  path: string;
  aspect: number;
}) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    invoke<string>("get_thumbnail_data_url", { file: path, size: 256 })
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
    <div
      className="bg-neutral-200 overflow-hidden rounded"
      style={{ aspectRatio: `${aspect}` }}
    >
      {src ? (
        <img
          src={src}
          alt=""
          className="w-full h-full object-cover"
          draggable={false}
        />
      ) : (
        <div className="w-full h-full flex items-center justify-center text-[10px] text-neutral-400">
          …
        </div>
      )}
    </div>
  );
}

export function RecommendPanel() {
  const records = useWehiStore((s) => s.analysisRecords);
  const selection = useWehiStore((s) => s.recommendationSelection);
  const toggleRec = useWehiStore((s) => s.toggleRecommendation);
  const setSelection = useWehiStore((s) => s.setRecommendationSelection);
  const applySelectionToQueue = useWehiStore((s) => s.applySelectionToQueue);
  const setStage = useWehiStore((s) => s.setStage);

  const [networks, setNetworks] = useState<NetworkSpec[]>([]);
  const [selectedNetwork, setSelectedNetwork] = useState<string | null>(null);
  const [recommendations, setRecommendations] = useState<Recommendation[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<NetworkSpec[]>("list_networks")
      .then((list) => {
        setNetworks(list);
        if (list.length > 0 && !selectedNetwork) {
          setSelectedNetwork(list[0].id);
        }
      })
      .catch((e) => setError(String(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!selectedNetwork) return;
    setLoading(true);
    setError(null);
    invoke<Recommendation[]>("get_recommendations", { network: selectedNetwork })
      .then(setRecommendations)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [selectedNetwork, records]);

  const network = networks.find((n) => n.id === selectedNetwork) ?? null;
  const aspect = network ? network.aspect_ratio : 1;

  // Mostramos TODAS las recomendaciones (ya vienen ordenadas
  // descendente por el backend). Filtrar las de score 0 escondía
  // demasiado y daba la sensación de que "no funciona".
  const visibles = recommendations;

  function exportToQueue() {
    if (selection.length === 0) return;
    const ok = window.confirm(
      `Reemplazar la cola con ${selection.length} archivo(s) seleccionado(s) y abrir la etapa Procesar?`,
    );
    if (!ok) return;
    applySelectionToQueue();
    setStage("procesar");
  }

  function selectAllVisible() {
    setSelection(visibles.map((r) => r.path));
  }

  function selectTop(n: number) {
    setSelection(visibles.slice(0, n).map((r) => r.path));
  }

  const allSelected =
    visibles.length > 0 && selection.length === visibles.length;

  return (
    <div className="h-full flex flex-col">
      <div className="px-3 py-2 border-b border-neutral-200 bg-white space-y-2">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-[11px] text-neutral-600 mr-1">Red social:</span>
          {networks.map((n) => (
            <button
              key={n.id}
              type="button"
              onClick={() => setSelectedNetwork(n.id)}
              className={`text-[11px] px-2 py-1 rounded border ${
                selectedNetwork === n.id
                  ? "bg-neutral-900 text-white border-neutral-900"
                  : "bg-white text-neutral-700 border-neutral-300 hover:bg-neutral-50"
              }`}
            >
              {n.name}
            </button>
          ))}
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-[11px] text-neutral-500">
            Seleccionadas: <span className="font-medium text-neutral-800">{selection.length}</span>{" "}
            de {visibles.length}
          </span>
          <button
            type="button"
            onClick={selectAllVisible}
            disabled={visibles.length === 0 || allSelected}
            className="text-[11px] px-2 py-1 bg-white border border-neutral-300 rounded hover:bg-neutral-50 disabled:opacity-50"
          >
            Seleccionar todas
          </button>
          <button
            type="button"
            onClick={() => selectTop(10)}
            disabled={visibles.length === 0}
            className="text-[11px] px-2 py-1 bg-white border border-neutral-300 rounded hover:bg-neutral-50 disabled:opacity-50"
          >
            Top 10
          </button>
          <button
            type="button"
            onClick={() => setSelection([])}
            disabled={selection.length === 0}
            className="text-[11px] px-2 py-1 bg-white border border-neutral-300 rounded hover:bg-neutral-50 disabled:opacity-50"
          >
            Limpiar
          </button>
          <div className="ml-auto flex items-center gap-2">
            {selection.length === 0 && visibles.length > 0 && (
              <span className="text-[10px] text-neutral-500 italic">
                Haz click en las tarjetas para seleccionar
              </span>
            )}
            <button
              type="button"
              onClick={exportToQueue}
              className="text-[11px] px-3 py-1 bg-neutral-900 text-white rounded hover:bg-neutral-700 disabled:opacity-50"
              disabled={selection.length === 0}
            >
              Exportar a Procesar →
            </button>
          </div>
        </div>
        <div className="text-[10px] text-neutral-500 italic">
          La recomendación combina calidad técnica y encaje de formato. NO
          predice alcance ni viralidad.
        </div>
      </div>

      {error && (
        <div className="px-3 py-1 text-[11px] text-red-600 break-all">
          {error}
        </div>
      )}

      <div className="flex-1 overflow-auto p-3 bg-neutral-50">
        {records.length === 0 ? (
          <div className="h-full flex flex-col items-center justify-center text-xs text-neutral-500 text-center p-6 gap-3">
            <div>
              Para ver recomendaciones primero ejecuta el análisis del lote.
            </div>
            <button
              type="button"
              onClick={() => setStage("depurar")}
              className="text-xs px-3 py-1.5 bg-neutral-900 text-white rounded hover:bg-neutral-700"
            >
              Ir a Depurar →
            </button>
          </div>
        ) : loading ? (
          <div className="text-xs text-neutral-400">Calculando recomendaciones…</div>
        ) : visibles.length === 0 ? (
          <div className="text-xs text-neutral-500">
            No hay recomendaciones para esta red. Prueba otra orientación o vuelve a analizar.
          </div>
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-3">
            {visibles.map((rec, idx) => {
              const checked = selection.includes(rec.path);
              return (
                <button
                  key={rec.path}
                  type="button"
                  onClick={() => toggleRec(rec.path)}
                  className={`text-left bg-white border rounded-lg overflow-hidden hover:shadow-md transition cursor-pointer relative ${
                    checked
                      ? "border-neutral-900 ring-2 ring-neutral-900"
                      : "border-neutral-200 hover:border-neutral-400"
                  }`}
                >
                  <div
                    className={`absolute top-2 left-2 z-10 w-5 h-5 rounded border-2 flex items-center justify-center text-white text-xs font-bold transition ${
                      checked
                        ? "bg-neutral-900 border-neutral-900"
                        : "bg-white/80 border-neutral-400"
                    }`}
                  >
                    {checked ? "✓" : ""}
                  </div>
                  <div className="absolute top-2 right-2 z-10 bg-black/50 text-white text-[10px] px-1.5 py-0.5 rounded">
                    #{idx + 1}
                  </div>
                  <ThumbCrop path={rec.path} aspect={aspect} />
                  <div className="px-2 py-1.5">
                    <div className="text-[11px] font-medium text-neutral-800 truncate">
                      {fileName(rec.path)}
                    </div>
                    <div className="text-[10px] text-neutral-500 flex items-center justify-between mt-0.5">
                      <span>rec {rec.recommendation.toFixed(2)}</span>
                      <span className="text-neutral-400">
                        fit {rec.fit.toFixed(2)}
                      </span>
                    </div>
                    <div className="text-[10px] text-neutral-400">
                      composite {rec.composite_score.toFixed(2)}
                    </div>
                  </div>
                </button>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
