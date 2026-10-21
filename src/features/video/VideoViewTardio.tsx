import { lazy, Suspense, type ComponentProps } from "react";

/**
 * Fork (Editor de vídeo): a tela (player, timeline, chat) só carrega quando o usuário abre. Fechada,
 * não custa nada: sem timer, sem poll, sem webview nem processo. Mesmo componente e props.
 */
const VideoViewReal = lazy(() => import("./VideoView").then((modulo) => ({ default: modulo.VideoView })));

type Props = ComponentProps<typeof import("./VideoView").VideoView>;

export function VideoView(props: Props) {
  return (
    <Suspense fallback={<div className="h-full p-4 font-mono text-xs text-text-muted">Abrindo edição de vídeo…</div>}>
      <VideoViewReal {...props} />
    </Suspense>
  );
}
