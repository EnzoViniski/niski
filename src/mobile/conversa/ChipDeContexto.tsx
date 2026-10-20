// Fork (Onda 2, itens 1 e 3): no topo da conversa, "Contexto 34% · ~US$ 0,82" quando a statusline da
// sessão manda os números (o custo é estimativa do Claude Code), e se os eventos do Claude chegam.
import type { GanchosDaConversa } from "../api";

const dolar = new Intl.NumberFormat("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 });

export function textoDoContexto(ganchos?: GanchosDaConversa | null): string | null {
  const contexto = ganchos?.contexto;
  if (!contexto) return null;
  const partes = [
    contexto.pct != null && `Contexto ${Math.round(contexto.pct)}%`,
    contexto.custo_usd != null && `~US$ ${dolar.format(contexto.custo_usd)}`,
  ].filter(Boolean);
  return partes.length ? partes.join(" · ") : null;
}

/** Rótulo curto do estado dos eventos, com a explicação no `title`. */
export function rotuloDosGanchos(ganchos?: GanchosDaConversa | null): { texto: string; dica: string } | null {
  switch (ganchos?.estado) {
    case "ativos": return { texto: "eventos ✓", dica: "O Claude Code está mandando eventos para o app (turno, permissões)." };
    case "sem_eventos": return { texto: "lendo o terminal",
      dica: "Nenhum evento do Claude Code ainda: versão antiga, pasta sem confiança ou política da máquina. O app segue lendo o terminal." };
    default: return null;
  }
}

export function ChipDeContexto({ ganchos }: { ganchos?: GanchosDaConversa | null }) {
  const texto = textoDoContexto(ganchos);
  const eventos = rotuloDosGanchos(ganchos);
  if (!texto && !eventos) return null;
  return <span className="ml-2 shrink-0 text-[11px] font-normal text-text-muted"
    title={[texto && "Contexto e custo lidos da statusline do Claude Code; o custo é estimado.", eventos?.dica].filter(Boolean).join("\n")}>
    {[texto, eventos?.texto].filter(Boolean).join(" · ")}
  </span>;
}
