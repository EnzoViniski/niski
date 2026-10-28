import { lazy, Suspense, type ComponentProps } from "react";

/**
 * A tela de Fluxos traz o React Flow (@xyflow, ~250 kB + CSS). Só carrega quando o usuário
 * abre a tela. Mesmo componente e props.
 */
const FluxosViewReal = lazy(() => import("./FluxosView").then((modulo) => ({ default: modulo.FluxosView })));

type Props = ComponentProps<typeof import("./FluxosView").FluxosView>;

export function FluxosView(props: Props) {
  return (
    <Suspense fallback={<div className="h-full p-4 font-mono text-xs text-text-muted">Abrindo fluxos…</div>}>
      <FluxosViewReal {...props} />
    </Suspense>
  );
}
