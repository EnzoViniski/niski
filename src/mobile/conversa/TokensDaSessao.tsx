// Total de tokens da conversa no topo da aba (pedido do Enzo, 21/10/2026). O engine soma o uso
// que a CLI grava (Claude: `usage` de cada resposta; Codex: o acumulado `token_count`) —
// `omni-core/src/tokens_da_sessao.rs`. Entrada inclui o que veio do cache.
export interface TokensDaSessao { total: number; entrada: number; cache: number; saida: number }

const inteiro = new Intl.NumberFormat("pt-BR");
const curto = new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 1 });

/** "850", "12,4 mil", "1,2 mi". */
export function tokensCurtos(n: number): string {
  if (n < 1_000) return inteiro.format(n);
  if (n < 1_000_000) return `${curto.format(n / 1_000)} mil`;
  return `${curto.format(n / 1_000_000)} mi`;
}

export function dicaDosTokens(t: TokensDaSessao): string {
  const cache = t.cache > 0 ? ` (${inteiro.format(t.cache)} do cache)` : "";
  return `Tokens desta conversa: ${inteiro.format(t.total)}\nEntrada: ${inteiro.format(t.entrada)}${cache}\nSaída: ${inteiro.format(t.saida)}`;
}

export function ChipDeTokens({ tokens }: { tokens?: TokensDaSessao | null }) {
  if (!tokens || tokens.total <= 0) return null;
  return (
    <span className="chip-tokens" title={dicaDosTokens(tokens)}>
      {tokensCurtos(tokens.total)} tokens
    </span>
  );
}
