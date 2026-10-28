import { LockIcon, NotesIcon, ShieldIcon, WarningIcon, ZapOffIcon } from "../components/ui/PixelIcon";
import "./conversaRica.css";

/**
 * Selo com o modo de permissão em que o agente está, junto do campo de mensagem — o mesmo
 * que o rodapé do Claude Code mostra ("plan mode on", "accept edits on") e o controle de permissões
 * do Codex. Vem do transcript (`permission-mode` do Claude, `turn_context` do Codex) e, com a
 * sessão aberta, do rodapé da tela, que responde na hora.
 */

type Tom = "neutro" | "plano" | "alerta";
interface Modo { rotulo: string; dica: string; tom: Tom }

export const MODOS_DE_PERMISSAO: Record<string, Modo> = {
  default: { rotulo: "Perguntar", dica: "Pede sua aprovação antes de editar arquivos e rodar comandos", tom: "neutro" },
  acceptEdits: { rotulo: "Aceitar edições", dica: "Edita arquivos sem perguntar; comandos ainda pedem aprovação", tom: "neutro" },
  plan: { rotulo: "Plano", dica: "Só lê e planeja; nada é editado até você aprovar o plano", tom: "plano" },
  auto: { rotulo: "Auto", dica: "As ações são revisadas automaticamente; só as arriscadas pedem você", tom: "neutro" },
  dontAsk: { rotulo: "Não perguntar", dica: "Só faz o que já está liberado; o resto é negado sem perguntar", tom: "neutro" },
  bypassPermissions: { rotulo: "Sem permissões", dica: "Faz tudo sem pedir aprovação", tom: "alerta" },
  semPerguntar: { rotulo: "Sem perguntar", dica: "Não pede aprovação (continua dentro da sandbox)", tom: "neutro" },
  acessoTotal: { rotulo: "Acesso total", dica: "Sem sandbox e sem pedir aprovação", tom: "alerta" },
  somenteLeitura: { rotulo: "Somente leitura", dica: "Só lê arquivos; não edita nada", tom: "neutro" },
};

/** O modo da tela (imediato) vale mais que o do transcript; sem nenhum dos dois, nada. */
export function modoVigente(daTela: string | null | undefined, doTranscript: string | null | undefined): string | null {
  return daTela || doTranscript || null;
}

export function SeloDoModo({ modo }: { modo: string | null | undefined }) {
  if (!modo) return null;
  const conhecido = MODOS_DE_PERMISSAO[modo];
  const { rotulo, dica, tom } = conhecido ?? { rotulo: modo, dica: "Modo de permissão do agente", tom: "neutro" as Tom };
  const Icone = tom === "alerta" ? WarningIcon : tom === "plano" ? NotesIcon : modo === "somenteLeitura" ? LockIcon
    : modo === "bypassPermissions" || modo === "semPerguntar" ? ZapOffIcon : ShieldIcon;
  return (
    <span className={`selo-modo is-${tom}`} title={`Modo de permissão: ${rotulo}. ${dica}`} aria-label={`Modo de permissão: ${rotulo}`}>
      <Icone aria-hidden className="h-3 w-3 shrink-0" />
      <span className="selo-modo-texto">{rotulo}</span>
    </span>
  );
}
