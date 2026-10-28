// "Trabalhou por 20m 34s ›" — o trabalho do pedido (falas intermediárias e ações) recolhido acima
// da resposta final; um clique abre o passo a passo e outro fecha (`turnos.ts`).
import type { ReactNode } from "react";
import { ChevronDownIcon } from "../../components/ui/PixelIcon";
import { duracaoDoTrabalho } from "./turnos";
import "./trabalhoRecolhido.css";

export function TrabalhoRecolhido({ duracaoMs, aberto, onAlternar, children }: {
  duracaoMs: number | null;
  aberto: boolean;
  onAlternar: () => void;
  children: ReactNode;
}) {
  const rotulo = duracaoMs !== null ? `Trabalhou por ${duracaoDoTrabalho(duracaoMs)}` : "Trabalho do agente";
  return (
    <div className={`trabalho${aberto ? " is-aberto" : ""}`}>
      <button type="button" className="trabalho-cabecalho" aria-expanded={aberto} onClick={onAlternar}
        title={aberto ? "Recolher o passo a passo" : "Ver o passo a passo deste pedido"}>
        <span>{rotulo}</span>
        <ChevronDownIcon aria-hidden className="trabalho-seta h-3.5 w-3.5" />
      </button>
      {aberto && <div className="trabalho-passos">{children}</div>}
    </div>
  );
}
