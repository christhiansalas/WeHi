// Tipos del frontend.
//
// **Los tipos que cruzan la frontera Rust↔TS vienen auto-generados**
// desde los structs Rust con `ts-rs`. Regenerar:
//
//     npm run gen:types
//
// (corre `cargo test --workspace` y limpia el dir duplicado).
//
// Aquí solo quedan:
// - Re-exports de los bindings generados.
// - Tipos exclusivos del frontend (no tienen contraparte en Rust).
// - Helpers (defaults + utilidades).

// ──── Tipos generados por ts-rs ────
export type { ImageAdjustments } from "./bindings/ImageAdjustments";
export type { LogoBackground } from "./bindings/LogoBackground";
export type { LogoConfig } from "./bindings/LogoConfig";
export type { LogoPosition } from "./bindings/LogoPosition";
export type { NetworkCrop } from "./bindings/NetworkCrop";
export type { OutputConfig } from "./bindings/OutputConfig";
export type { OutputFormat } from "./bindings/OutputFormat";
export type { Preset } from "./bindings/Preset";
export type { ResizeConfig } from "./bindings/ResizeConfig";
export type { ResizeMode } from "./bindings/ResizeMode";
export type { TextWatermark } from "./bindings/TextWatermark";

export type { AestheticScore } from "./bindings/AestheticScore";
export type { AnalysisRecord } from "./bindings/AnalysisRecord";
export type { CullStatus } from "./bindings/CullStatus";
export type { DedupConfig } from "./bindings/DedupConfig";
export type { FaceSummary } from "./bindings/FaceSummary";
export type { NetworkOrientation } from "./bindings/NetworkOrientation";
export type { NetworkSpec } from "./bindings/NetworkSpec";
export type { QualityMetrics } from "./bindings/QualityMetrics";
export type { ScoringWeights } from "./bindings/ScoringWeights";
export type { SubScores } from "./bindings/SubScores";

export type { ActiveBatch } from "./bindings/ActiveBatch";
export type { AnalysisDoneEvent } from "./bindings/AnalysisDoneEvent";
export type { AnalysisFileError } from "./bindings/AnalysisFileError";
export type { AnalysisProgressEvent } from "./bindings/AnalysisProgressEvent";
export type { BatchDoneEvent } from "./bindings/BatchDoneEvent";
export type { FfmpegStatus } from "./bindings/FfmpegStatus";
export type { FileAppearedEvent } from "./bindings/FileAppearedEvent";
export type { FileDoneEvent } from "./bindings/FileDoneEvent";
export type { FileError } from "./bindings/FileError";
export type { FontStatus } from "./bindings/FontStatus";
export type { LogoEntry } from "./bindings/LogoEntry";
export type { Metrics } from "./bindings/Metrics";
export type { MoveRejectsReport } from "./bindings/MoveRejectsReport";
export type { PreviewResult } from "./bindings/PreviewResult";
export type { ProgressEvent } from "./bindings/ProgressEvent";
export type { Recommendation } from "./bindings/Recommendation";
export type { VideoProgressEvent } from "./bindings/VideoProgressEvent";
export type { WatchStatus } from "./bindings/WatchStatus";

// Atajos locales que el resto de los componentes ya importan por nombre.
import type { Preset } from "./bindings/Preset";
import type { LogoConfig } from "./bindings/LogoConfig";
import type { ImageAdjustments } from "./bindings/ImageAdjustments";
import type { ScoringWeights } from "./bindings/ScoringWeights";
import type { DedupConfig } from "./bindings/DedupConfig";

// ──── Tipos exclusivos del frontend (sin contraparte Rust) ────

export type QueueItemStatus = "pendiente" | "procesando" | "ok" | "error";

export interface QueueItem {
  path: string;
  name: string;
  status: QueueItemStatus;
  error?: string | null;
}

/** Estado de los lotes (procesamiento o análisis). */
export type BatchState = "idle" | "running" | "done" | "cancelled";

export type Stage = "procesar" | "depurar" | "recomendar";

/** Lo que persistimos a disco entre sesiones. Lo escribimos como
 *  JSON arbitrario; cada campo es opcional. */
export interface Preferences {
  last_source_folder?: string;
  last_output_folder?: string;
  last_preset_name?: string;
  weights?: ScoringWeights;
  max_threads?: number;
  include_undecided?: boolean;
  use_ai?: boolean;
}

/** Forma con que serde serializa SystemTime a JSON. */
export interface SystemTimeRepr {
  secs_since_epoch?: number;
  nanos_since_epoch?: number;
}

// ──── Helpers ────

const VIDEO_EXTS = ["mp4", "mov", "m4v", "webm", "mkv", "avi"] as const;

export function isVideoPath(path: string): boolean {
  const i = path.lastIndexOf(".");
  if (i < 0) return false;
  const ext = path.slice(i + 1).toLowerCase();
  return (VIDEO_EXTS as readonly string[]).includes(ext);
}

export function allLogos(p: Preset): LogoConfig[] {
  const result: LogoConfig[] = [];
  if (p.logo) result.push(p.logo);
  for (const l of p.logos) result.push(l);
  return result;
}

export function defaultPreset(): Preset {
  return {
    name: "Por defecto",
    resize: {
      mode: "MaxDimension",
      width: 2048,
      height: 2048,
      percentage: null,
      keep_aspect_ratio: true,
      allow_upscale: false,
    },
    logo: null,
    logos: [],
    output: {
      format: "Jpeg",
      quality: 85,
      folder: "",
      filename_pattern: "{name}{ext}",
      preserve_structure: false,
    },
    network_crops: [],
    text_watermarks: [],
    adjustments: defaultAdjustments(),
  };
}

export function defaultWeights(): ScoringWeights {
  return {
    w_sharpness: 0.3,
    w_exposure: 0.22,
    w_aesthetic: 0.38,
    w_resolution: 0.1,
    face_bonus_max: 0.1,
  };
}

export function defaultDedup(): DedupConfig {
  return { hamming_threshold: 10, time_window_seconds: 30 };
}

export function defaultAdjustments(): ImageAdjustments {
  return { brightness: 0, contrast: 0, saturation: 0, temperature: 0 };
}
