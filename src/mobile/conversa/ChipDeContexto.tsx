// Fork (Onda 2, item 3): "Contexto 34% · ~US$ 0,82" no topo da conversa, quando a statusline da sessão
// manda os números (conta com "Ler uso pela statusline" ligado). O custo é estimativa do Claude Code.
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

export function ChipDeContexto({ ganchos }: { ganchos?: GanchosDaConversa | null }) {
  const texto = textoDoContexto(ganchos);
  if (!texto) return null;
  return <span className="ml-2 shrink-0 text-[11px] font-normal text-text-muted" title="Lido da statusline do Claude Code; o custo é estimado">
    {texto}
  </span>;
}
