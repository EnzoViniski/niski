import { AlertIcon, CheckIcon, CircleIcon, PlayIcon, ReloadIcon, ZapOffIcon } from "./ui/PixelIcon";
import { AgentIcon } from "./ui/AgentIcon";
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
 * - **retoma** (fork, retomar-sempre): agente que o engine anterior deixou aberto; volta com
 *   `--resume` quando a aba é aberta.
 */
export type ConversaStatus = "trabalhando" | "aprovacao" | "interrompido" | "finalizado" | "retoma" | "neutro";

export function statusDaConversa(session: Pick<TerminalSession, "state" | "notice" | "retoma_ao_abrir">, naoVista: boolean): ConversaStatus {
  if (session.state === "approval_required") return "aprovacao";
  if (session.state === "working") return "trabalhando";
  if (session.retoma_ao_abrir) return "retoma";
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
  retoma: { icon: ReloadIcon, cor: "text-text-secondary", rotulo: "retoma ao abrir",
    selo: "bg-overlay/5 border-border-subtle", linha: "border-transparent" },
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

/** Ordem dos selos no resumo: o que pede você primeiro. */
const ORDEM: ConversaStatus[] = ["aprovacao", "interrompido", "trabalhando", "finalizado", "retoma", "neutro"];

/**
 * Resumo dos agentes de um projeto/workspace: um selo por (agente, status) com ícone, cor do status
 * e quantos estão naquilo. Sessões encerradas ficam de fora — não estão "ali" de verdade.
 */
export function ResumoDeAgentes({ sessions, naoVistas }: { sessions: TerminalSession[]; naoVistas: Set<string> }) {
  const grupos = new Map<string, { provider: string; status: ConversaStatus; total: number }>();
  for (const session of sessions) {
    if (!session.provider || session.state === "stopped" || session.state === "orphan") continue;
    const status = statusDaConversa(session, naoVistas.has(session.conversation_id || session.id));
    const chave = `${session.provider}|${status}`;
    const grupo = grupos.get(chave) ?? { provider: session.provider, status, total: 0 };
    grupo.total += 1;
    grupos.set(chave, grupo);
  }
  if (grupos.size === 0) return <span className="text-[10px] leading-4 text-text-muted">sem agentes</span>;
  const lista = [...grupos.values()].sort((a, b) => ORDEM.indexOf(a.status) - ORDEM.indexOf(b.status));
  return (
    <span className="flex min-w-0 flex-wrap items-center gap-1">
      {lista.map(({ provider, status, total }) => (
        <span
          key={`${provider}|${status}`}
          title={`${total} ${provider}${VISUAL[status].rotulo ? ` · ${VISUAL[status].rotulo}` : ""}`}
          className={`inline-flex h-4 items-center gap-1 border px-1 text-[9px] font-bold leading-none tabular-nums ${
            status === "neutro" ? "border-border-subtle text-text-muted" : `${VISUAL[status].cor} ${VISUAL[status].selo}`
          }`}
        >
          <AgentIcon provider={provider} size={10} />
          {total}
        </span>
      ))}
    </span>
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
