import { create } from "zustand";

import type {
  AnalysisRecord,
  BatchState,
  CullStatus,
  DedupConfig,
  LogoEntry,
  Preset,
  QueueItem,
  QueueItemStatus,
  ScoringWeights,
  Stage,
} from "./types";
import { defaultDedup, defaultPreset, defaultWeights } from "./types";

interface WehiStore {
  // Carga
  sourceFolder: string | null;
  queue: QueueItem[];
  selectedPath: string | null;

  // Configuración
  preset: Preset;
  logos: LogoEntry[];

  // Estado del lote
  batchState: BatchState;
  procesadas: number;
  total: number;
  archivoActual: string | null;
  errores: { ruta: string; mensaje: string }[];
  videoProgress: { ruta: string; currentUs: number; totalUs: number } | null;

  // Etapa actual (procesar / depurar / recomendar)
  stage: Stage;

  // Análisis
  analysisRecords: AnalysisRecord[];
  analysisRunning: boolean;
  analysisProgress: { analizadas: number; total: number; archivo: string | null };
  weights: ScoringWeights;

  // Acciones
  setSourceFolder: (folder: string) => void;
  setQueueFromPaths: (paths: string[]) => void;
  selectPath: (path: string | null) => void;
  setItemStatus: (
    path: string,
    status: QueueItemStatus,
    error?: string | null,
  ) => void;
  setPreset: (preset: Preset) => void;
  patchPreset: (patch: Partial<Preset>) => void;
  setLogos: (logos: LogoEntry[]) => void;
  startBatch: (total: number) => void;
  updateProgress: (procesadas: number, archivo: string) => void;
  updateVideoProgress: (ruta: string, currentUs: number, totalUs: number) => void;
  finishBatch: (cancelled: boolean) => void;
  setBatchErrors: (errores: { ruta: string; mensaje: string }[]) => void;
  resetBatch: () => void;

  setStage: (stage: Stage) => void;

  setAnalysisRecords: (records: AnalysisRecord[]) => void;
  setAnalysisRunning: (running: boolean) => void;
  setAnalysisProgress: (p: {
    analizadas: number;
    total: number;
    archivo: string | null;
  }) => void;
  setLocalCullStatus: (path: string, status: CullStatus) => void;
  setWeights: (w: ScoringWeights) => void;
  patchWeights: (patch: Partial<ScoringWeights>) => void;

  dedupConfig: DedupConfig;
  setDedupConfig: (d: DedupConfig) => void;
  patchDedupConfig: (patch: Partial<DedupConfig>) => void;

  recommendationSelection: string[];
  toggleRecommendation: (path: string) => void;
  setRecommendationSelection: (paths: string[]) => void;
  applySelectionToQueue: () => void;

  batchFiles: QueueItem[];
  setBatchFiles: (files: QueueItem[]) => void;
  setBatchFileStatus: (
    path: string,
    status: QueueItemStatus,
    error?: string | null,
  ) => void;
}

function fileName(p: string): string {
  const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
  return i >= 0 ? p.slice(i + 1) : p;
}

export const useWehiStore = create<WehiStore>((set) => ({
  sourceFolder: null,
  queue: [],
  selectedPath: null,
  preset: defaultPreset(),
  logos: [],
  batchState: "idle",
  procesadas: 0,
  total: 0,
  archivoActual: null,
  errores: [],
  videoProgress: null,
  stage: "procesar",
  analysisRecords: [],
  analysisRunning: false,
  analysisProgress: { analizadas: 0, total: 0, archivo: null },
  weights: defaultWeights(),
  dedupConfig: defaultDedup(),
  recommendationSelection: [],

  setSourceFolder: (folder) => set({ sourceFolder: folder }),

  setQueueFromPaths: (paths) =>
    set({
      queue: paths.map((p) => ({
        path: p,
        name: fileName(p),
        status: "pendiente" as QueueItemStatus,
      })),
      selectedPath: paths[0] ?? null,
    }),

  selectPath: (path) => set({ selectedPath: path }),

  setItemStatus: (path, status, error) =>
    set((state) => ({
      queue: state.queue.map((item) =>
        item.path === path ? { ...item, status, error } : item,
      ),
    })),

  setPreset: (preset) => set({ preset }),

  patchPreset: (patch) =>
    set((state) => ({ preset: { ...state.preset, ...patch } })),

  setLogos: (logos) => set({ logos }),

  startBatch: (total) =>
    set({
      batchState: "running",
      procesadas: 0,
      total,
      archivoActual: null,
      errores: [],
      videoProgress: null,
    }),

  updateProgress: (procesadas, archivo) =>
    set({ procesadas, archivoActual: archivo }),

  updateVideoProgress: (ruta, currentUs, totalUs) =>
    set({ videoProgress: { ruta, currentUs, totalUs } }),

  finishBatch: (cancelled) =>
    set({
      batchState: cancelled ? "cancelled" : "done",
      archivoActual: null,
      videoProgress: null,
    }),

  setBatchErrors: (errores) => set({ errores }),

  resetBatch: () =>
    set({
      batchState: "idle",
      procesadas: 0,
      total: 0,
      archivoActual: null,
      errores: [],
      videoProgress: null,
    }),

  setStage: (stage) => set({ stage }),

  setAnalysisRecords: (records) => set({ analysisRecords: records }),

  setAnalysisRunning: (running) => set({ analysisRunning: running }),

  setAnalysisProgress: (p) => set({ analysisProgress: p }),

  setLocalCullStatus: (path, status) =>
    set((state) => ({
      analysisRecords: state.analysisRecords.map((r) =>
        r.path === path ? { ...r, status } : r,
      ),
    })),

  setWeights: (w) => set({ weights: w }),
  patchWeights: (patch) => set((s) => ({ weights: { ...s.weights, ...patch } })),

  setDedupConfig: (d) => set({ dedupConfig: d }),
  patchDedupConfig: (patch) =>
    set((s) => ({ dedupConfig: { ...s.dedupConfig, ...patch } })),

  batchFiles: [],
  setBatchFiles: (files) => set({ batchFiles: files }),
  setBatchFileStatus: (path, status, error) =>
    set((s) => ({
      batchFiles: s.batchFiles.map((f) =>
        f.path === path ? { ...f, status, error } : f,
      ),
    })),

  toggleRecommendation: (path) =>
    set((s) => {
      const has = s.recommendationSelection.includes(path);
      return {
        recommendationSelection: has
          ? s.recommendationSelection.filter((p) => p !== path)
          : [...s.recommendationSelection, path],
      };
    }),

  setRecommendationSelection: (paths) =>
    set({ recommendationSelection: paths }),

  applySelectionToQueue: () =>
    set((s) => {
      const sel = new Set(s.recommendationSelection);
      if (sel.size === 0) return {} as Partial<WehiStore>;
      const filtered = s.queue.filter((q) => sel.has(q.path));
      return {
        queue: filtered,
        selectedPath: filtered[0]?.path ?? null,
      };
    }),
}));
