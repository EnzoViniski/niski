import { lazy, Suspense } from "react";

// Fork (Tarefas e Rotina no celular): as telas só baixam no celular quando são abertas.
const TarefasReal = lazy(() => import("./Tarefas").then((m) => ({ default: m.Tarefas })));
const RotinaReal = lazy(() => import("./Rotina").then((m) => ({ default: m.Rotina })));

export function TarefasNoCelular(props: { voltar: () => void }) {
  return <Suspense fallback={<p className="m-vazio">Abrindo as tarefas…</p>}><TarefasReal {...props} /></Suspense>;
}

export function RotinaNoCelular(props: { abrirConversaNoChat: (id: string) => void }) {
  return <Suspense fallback={<p className="m-vazio">Abrindo a rotina…</p>}><RotinaReal {...props} /></Suspense>;
}
