import { lazy, Suspense, type ComponentProps } from "react";

/**
 * Fork: o fluxo ao vivo traz o React Flow (@xyflow, ~250 kB + CSS). Só carrega quando a primeira aba
 * de fluxo abre, como `fluxos/FluxosViewTardio`. Mesmo componente e props.
 */
const FluxoAoVivoReal = lazy(() => import("./FluxoAoVivo").then((modulo) => ({ default: modulo.FluxoAoVivo })));

type Props = ComponentProps<typeof import("./FluxoAoVivo").FluxoAoVivo>;

export function FluxoAoVivo(props: Props) {
  return (
    <Suspense fallback={<div className="h-full p-4 font-mono text-xs text-text-muted">Abrindo o fluxo…</div>}>
      <FluxoAoVivoReal {...props} />
    </Suspense>
  );
}
