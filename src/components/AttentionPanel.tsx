import { CloseIcon } from "./ui/PixelIcon";
import { ATTENTION_LABEL, type AttentionItem, type AttentionReason } from "../features/terminal/useAttention";
import { EtiquetaDoStatus, IconeDoStatus, type ConversaStatus } from "./ConversaStatus";

/** Motivo do engine → status da conversa, o mesmo vocabulário da lista de agentes. */
const STATUS: Record<AttentionReason, ConversaStatus> = {
  approval_required: "aprovacao",
  usage_limit: "interrompido",
  api_error: "interrompido",
  crashed: "interrompido",
  orphan: "neutro",
  answered: "finalizado",
  stopped: "neutro",
};

/** O detalhe que o status não diz: por que foi interrompido. */
const DETALHE: Partial<Record<AttentionReason, string>> = {
  usage_limit: "limite de uso",
  api_error: "erro de API",
  crashed: "travou",
  orphan: "sessão anterior",
  stopped: "encerrado",
};

/**
 * Attention Center (spec §11.3) na sidebar: o único lugar do app que mostra agentes de TODOS os
 * workspaces, inclusive os que não estão abertos. Fica como primeiro filho do `<nav>`, e não como
 * `SidebarSection`, porque aquelas seções são desabilitadas sem projeto ativo — e um agente de
 * outro workspace pedindo aprovação tem de aparecer justamente quando você não está nele.
 */
export function AttentionPanel({
  items,
  onFocusSession,
  onDismiss,
}: {
  items: AttentionItem[];
  onFocusSession: (item: AttentionItem) => void;
  onDismiss: (item: AttentionItem) => void;
}) {
  if (items.length === 0) return null;

  return (
    <div className="space-y-0.5">
      <div className="flex items-center justify-between px-3 pt-1">
        <p className="text-text-muted text-[10px] font-medium uppercase tracking-wider">Atenção</p>
        <span
          aria-label={`${items.length} ${items.length === 1 ? "agente esperando" : "agentes esperando"} no total`}
          className="badge-pulse grid h-4 min-w-4 place-items-center bg-danger px-1 text-[10px] font-bold leading-none text-bg-primary"
        >
          {items.length > 9 ? "9+" : items.length}
        </span>
      </div>
      {items.map((item) => {
        const status = STATUS[item.reason];
        const detalhe = DETALHE[item.reason];
        return (
          <div key={item.sessionId} className="group relative">
          <button
            type="button"
            data-nav-item
            onClick={() => onFocusSession(item)}
            title={`${item.workspaceName} · ${item.projectName} · ${item.sessionName} — ${ATTENTION_LABEL[item.reason]}`}
            className="glass-hover w-full py-1.5 pl-3 pr-7 text-left rounded-none focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
          >
            <span className="flex min-w-0 items-center gap-2">
              <IconeDoStatus status={status} />
              <span className="min-w-0 flex-1 truncate text-xs text-text-secondary">{item.sessionName}</span>
              {/* O X de dispensar aparece no mesmo canto no hover. */}
              <EtiquetaDoStatus status={status} className="group-hover:invisible" />
            </span>
            <span className="mt-0.5 block truncate pl-5 text-[10px] text-text-muted">
              {item.workspaceName} · {item.projectName}{detalhe ? ` · ${detalhe}` : ""}
            </span>
          </button>
          <button
            type="button"
            onClick={() => onDismiss(item)}
            aria-label={`Dispensar aviso de ${item.sessionName}`}
            title="Dispensar aviso"
            className="absolute right-1.5 top-1/2 -translate-y-1/2 grid h-5 w-5 place-items-center text-text-muted opacity-0 transition-opacity hover:text-text-primary focus-visible:opacity-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent group-hover:opacity-100"
          >
            <CloseIcon className="h-3 w-3" aria-hidden="true" />
          </button>
          </div>
        );
      })}
    </div>
  );
}
