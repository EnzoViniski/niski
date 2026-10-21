import { lazy, Suspense } from "react";

// Fork (Roadmaps): a tela (FSRS, markdown) só baixa no celular quando é aberta.
const Real = lazy(() => import("./Roadmaps").then((m) => ({ default: m.Roadmaps })));

export function RoadmapsNoCelular(props: { roadmap: string | null; abrir: (id: string | null) => void }) {
  return <Suspense fallback={<p className="m-vazio">Abrindo roadmaps…</p>}><Real {...props} /></Suspense>;
}
