// Fork (Onda 2.0): saiu do `MobileApp.tsx` sem mudança de lógica.
import { useState } from "react";
import { NotesIcon, PlayIcon } from "../../components/ui/PixelIcon";

/** Plano e automático — o que o `shift+tab` alterna no terminal; os dois do dia a dia. Travado sem
 *  tela reconhecida: tecla no meio de um turno não é para o agente. */
export function ModoDoAgente({ modo, pronto, onModo }: { modo?: string | null; pronto: boolean; onModo: (alvo: "plan" | "auto") => Promise<void> }) {
  // O terminal leva um instante para confirmar a troca: o botão escolhido acende na hora (e pulsa
  // até a confirmação), em vez de o clique parecer ignorado.
  const [pedido, setPedido] = useState<"plan" | "auto" | null>(null);
  const ativo = pedido ?? modo;
  return <div className="composer-modos" role="group" aria-label="Modo do agente" data-ativo={ativo ?? undefined}>
    {(["plan", "auto"] as const).map((alvo) => (
      <button key={alvo} type="button" className={`composer-modo${ativo === alvo ? " is-ativo" : ""}${pedido === alvo ? " is-pedido" : ""}`}
        aria-pressed={ativo === alvo}
        disabled={!pronto && pedido === null}
        onClick={() => {
          if (pedido || alvo === modo) return;
          setPedido(alvo);
          void onModo(alvo).finally(() => setPedido(null));
        }}
        title={alvo === "plan" ? "Plano: o agente planeja e pergunta antes de mexer" : "Auto: o agente executa sem pedir aprovação"}>
        {alvo === "plan" ? <NotesIcon aria-hidden className="h-3.5 w-3.5" /> : <PlayIcon aria-hidden className="h-3.5 w-3.5" />}
        <span>{alvo === "plan" ? "Plano" : "Auto"}</span>
      </button>
    ))}
  </div>;
}
