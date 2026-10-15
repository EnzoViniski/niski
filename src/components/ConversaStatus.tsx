import { AlertIcon, CheckIcon, CircleIcon, PlayIcon, ZapOffIcon } from "./ui/PixelIcon";
import type { TerminalSession } from "../features/terminal/terminalService";

/**
 * O que a conversa está fazendo, em uma palavra — o que a lateral mostra de cada uma. São poucos de
 * propósito: o estado cru do engine tem seis valores mais dois avisos, e "answered" (parado esperando
 * você) era igual tanto para "acabou de responder" quanto para "nada novo desde ontem".
 *
 * - **trabalhando**: o agente está no meio de um turno.
 * - **aprovação**: parado num diálogo esperando você aprovar.
 * - **interrompido**: parou sem terminar — limite de uso, erro de API ou o processo caiu.
 * - **finalizado**: terminou um turno que você ainda não viu.
 * - **neutro**: nada para ver.
 */
export type ConversaStatus = "trabalhando" | "aprovacao" | "interrompido" | "finalizado" | "neutro";

export function statusDaConversa(session: Pick<TerminalSession, "state" | "notice">, naoVista: boolean): ConversaStatus {
  if (session.state === "approval_required") return "aprovacao";
  if (session.state === "working") return "trabalhando";
  if (session.notice || session.state === "crashed") return "interrompido";
  if (naoVista && session.state === "answered") return "finalizado";
  return "neutro";
}

const VISUAL: Record<
  ConversaStatus,
  {
    icon: React.ComponentType<{ className?: string; "aria-hidden"?: boolean }>;
    cor: string;
    rotulo: string;
    /** Selo da etiqueta: fundo e borda na cor do status. */
    selo: string;
    /** Linha da lateral: barra à esquerda e um fundo leve, para o status se ver de longe. */
    linha: string;
  }
> = {
  trabalhando: { icon: PlayIcon, cor: "text-success", rotulo: "trabalhando",
    selo: "bg-success/15 border-success/50", linha: "border-success bg-success/[0.07]" },
  aprovacao: { icon: AlertIcon, cor: "text-warning", rotulo: "aprovar",
    selo: "bg-warning/15 border-warning/50", linha: "border-warning bg-warning/[0.08]" },
  interrompido: { icon: ZapOffIcon, cor: "text-danger", rotulo: "interrompido",
    selo: "bg-danger/15 border-danger/50", linha: "border-danger bg-danger/[0.08]" },
  finalizado: { icon: CheckIcon, cor: "text-accent", rotulo: "finalizado",
    selo: "bg-accent/15 border-accent/50", linha: "border-accent bg-accent/[0.07]" },
  neutro: { icon: CircleIcon, cor: "text-text-muted", rotulo: "", selo: "", linha: "border-transparent" },
};

/** Classes da linha inteira (use com `border-l-2`). Neutro fica sem barra nem fundo. */
export function destaqueDaLinha(status: ConversaStatus): string {
  return VISUAL[status].linha;
}

export function rotuloDoStatus(status: ConversaStatus): string {
  return VISUAL[status].rotulo;
}

export function IconeDoStatus({ status, className = "" }: { status: ConversaStatus; className?: string }) {
  const { icon: Icon, cor } = VISUAL[status];
  return (
    <Icon
      className={`h-3.5 w-3.5 shrink-0 ${cor} ${status === "trabalhando" ? "animate-pulse" : ""} ${status === "neutro" ? "opacity-50" : ""} ${className}`}
      aria-hidden
    />
  );
}

/** Rótulo curto à direita do nome. Neutro não ganha texto: é o normal, e repetir "neutro" em toda
 *  linha só faria os outros estados se destacarem menos. */
export function EtiquetaDoStatus({ status, className = "" }: { status: ConversaStatus; className?: string }) {
  const { cor, rotulo, selo } = VISUAL[status];
  if (!rotulo) return null;
  return (
    <span className={`shrink-0 border px-1.5 py-px text-[9px] font-bold uppercase leading-[14px] tracking-wider ${cor} ${selo} ${className}`}>
      {rotulo}
    </span>
  );
}
