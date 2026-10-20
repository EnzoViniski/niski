import { useEffect, useState } from "react";
import { escolherConta, type EscolhaDaFila } from "./roteamentoService";

/** Fork (3B): as linhas que o engine devolve ("por que esta conta"), uma por conta avaliada. */
export function PorQueEstaConta({ escolha, className }: { escolha: EscolhaDaFila; className?: string }) {
  return (
    <ul className={["space-y-0.5 text-[11px] text-text-secondary", className].filter(Boolean).join(" ")} aria-label="Por que esta conta">
      {escolha.explicacao.map((linha, indice) => (
        <li key={indice} className={linha.includes("escolhida") ? "text-text-primary" : undefined}>{linha}</li>
      ))}
    </ul>
  );
}

export interface ContaAutomatica {
  escolha: EscolhaDaFila | null;
  carregando: boolean;
  erro: string | null;
}

/** A escolha da fila para `provider` neste projeto, consultada só enquanto `ativa` (a opção
 *  "Automática" está marcada). Engine antigo, sem a rota: erro em vez de conta inventada. */
export function useContaAutomatica(provider: string, projeto: string, ativa: boolean): ContaAutomatica {
  const [estado, setEstado] = useState<ContaAutomatica>({ escolha: null, carregando: false, erro: null });
  useEffect(() => {
    if (!ativa) { setEstado({ escolha: null, carregando: false, erro: null }); return; }
    let vivo = true;
    setEstado({ escolha: null, carregando: true, erro: null });
    escolherConta(provider, projeto)
      .then((escolha) => { if (vivo) setEstado({ escolha, carregando: false, erro: null }); })
      .catch((motivo) => {
        if (vivo) setEstado({ escolha: null, carregando: false, erro: motivo instanceof Error ? motivo.message : String(motivo) });
      });
    return () => { vivo = false; };
  }, [provider, projeto, ativa]);
  return estado;
}
