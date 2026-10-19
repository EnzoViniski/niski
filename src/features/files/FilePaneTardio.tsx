import { lazy, Suspense, type ComponentProps } from "react";

/**
 * Fork: o `FilePane` arrasta o Monaco inteiro (~4 MB do bundle principal, mais o CSS dele). Quase
 * toda abertura do app nem abre arquivo, então ele só baixa/parseia quando a primeira aba de
 * arquivo (ou a edição de um Markdown) aparece. Mesmo componente e props, só que tardio.
 */
const FilePaneReal = lazy(() => import("./FilePane").then((modulo) => ({ default: modulo.FilePane })));

type Props = ComponentProps<typeof import("./FilePane").FilePane>;

export function FilePane(props: Props) {
  return (
    <Suspense fallback={<div className="h-full p-4 font-mono text-xs text-text-muted">Abrindo editor…</div>}>
      <FilePaneReal {...props} />
    </Suspense>
  );
}
