import { AnalyzePanel } from "./components/AnalyzePanel";
import { DropZone } from "./components/DropZone";
import { FileQueue } from "./components/FileQueue";
import { MetricsPanel } from "./components/MetricsPanel";
import { PresetEditor } from "./components/PresetEditor";
import { PreviewPanel } from "./components/PreviewPanel";
import { ProcessControls } from "./components/ProcessControls";
import { ProgressPanel } from "./components/ProgressPanel";
import { RecommendPanel } from "./components/RecommendPanel";
import { ResumeBanner } from "./components/ResumeBanner";
import { TetheredControl } from "./components/TetheredControl";
import { useBatchEvents } from "./hooks/useBatchEvents";
import { usePersistedPreferences } from "./hooks/usePersistedPreferences";
import { useWehiStore } from "./store";
import type { Stage } from "./types";

const STAGES: { id: Stage; label: string }[] = [
  { id: "procesar", label: "Procesar" },
  { id: "depurar", label: "Depurar" },
  { id: "recomendar", label: "Recomendar" },
];

function WehiLogo() {
  return (
    <svg
      viewBox="0 0 40 40"
      width="40"
      height="40"
      xmlns="http://www.w3.org/2000/svg"
      className="shrink-0"
      aria-label="WeHi"
      role="img"
    >
      <rect width="40" height="40" rx="9" fill="#0f3a32" />
      <circle
        cx="16.5"
        cy="20"
        r="6"
        fill="none"
        stroke="#3ed6c5"
        strokeWidth="2.4"
      />
      <circle
        cx="23.5"
        cy="20"
        r="6"
        fill="none"
        stroke="#f5c93b"
        strokeWidth="2.4"
      />
    </svg>
  );
}

function StageTabs() {
  const stage = useWehiStore((s) => s.stage);
  const setStage = useWehiStore((s) => s.setStage);
  return (
    <div className="flex gap-1">
      {STAGES.map((s) => (
        <button
          key={s.id}
          type="button"
          onClick={() => setStage(s.id)}
          className={`text-xs px-3 py-1 rounded ${
            stage === s.id
              ? "bg-neutral-900 text-white"
              : "bg-white border border-neutral-300 text-neutral-700"
          }`}
        >
          {s.label}
        </button>
      ))}
    </div>
  );
}

function App() {
  useBatchEvents();
  usePersistedPreferences();
  const stage = useWehiStore((s) => s.stage);
  const batchState = useWehiStore((s) => s.batchState);
  const batchFiles = useWehiStore((s) => s.batchFiles);
  const showProgress = batchState !== "idle" || batchFiles.length > 0;

  return (
    <div className="h-screen flex flex-col bg-neutral-50 text-neutral-900">
      <header className="px-6 py-3 border-b border-neutral-200 bg-white flex items-center justify-between">
        <div className="flex items-center gap-3">
          <WehiLogo />
          <div>
            <h1 className="text-xl font-semibold tracking-tight leading-none">
              WeHi
            </h1>
            <p className="text-xs text-neutral-500 mt-0.5">
              Fotografía en masa — curar y procesar.
            </p>
          </div>
        </div>
        <StageTabs />
      </header>
      <ResumeBanner />
      <div className="flex-1 grid grid-cols-12 gap-4 p-4 min-h-0">
        <aside className="col-span-3 flex flex-col gap-3 min-h-0">
          <DropZone />
          {stage === "procesar" && <TetheredControl />}
          <FileQueue />
          {stage === "procesar" && <ProcessControls />}
          {stage === "depurar" && <MetricsPanel />}
        </aside>
        {stage === "procesar" && (
          <>
            <main className="col-span-6 border border-neutral-200 rounded-lg bg-white min-h-0 overflow-hidden">
              {showProgress ? <ProgressPanel /> : <PreviewPanel />}
            </main>
            <aside className="col-span-3 border border-neutral-200 rounded-lg bg-white min-h-0 overflow-hidden">
              <PresetEditor />
            </aside>
          </>
        )}
        {stage === "depurar" && (
          <main className="col-span-9 border border-neutral-200 rounded-lg bg-white min-h-0 overflow-hidden">
            <AnalyzePanel />
          </main>
        )}
        {stage === "recomendar" && (
          <main className="col-span-9 border border-neutral-200 rounded-lg bg-white min-h-0 overflow-hidden">
            <RecommendPanel />
          </main>
        )}
      </div>
    </div>
  );
}

export default App;
