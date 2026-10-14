import { useState } from "react";
import { AgentIcon } from "../components/ui/AgentIcon";
import { request } from "../mobile/api";
import { usePoll } from "../mobile/usePoll";
import { resetShort, usageTone, type UsageWindow } from "../lib/uso";

interface Conta {
  id: string;
  provider: string;
  name: string;
  usage: { status: string; primary: UsageWindow | null; secondary: UsageWindow | null; reason: string | null } | null;
}

const AGENTE: Record<string, string> = { claude: "Claude", codex: "Codex", cursor: "Cursor" };

/** O uso muda devagar (o PC relê a cada poucos minutos): um minuto entre leituras basta. */
const INTERVALO_MS = 60_000;

function Janela({ rotulo, janela }: { rotulo: string; janela: UsageWindow | null }) {
  if (!janela) return <span className="w-uso-janela"><span>{rotulo}</span><span className="w-uso-pct">—</span></span>;
  const usado = Math.min(100, Math.max(0, janela.used_percent));
  const tom = usageTone(janela.used_percent);
  const reset = resetShort(janela);
  return (
    <span className="w-uso-janela" title={reset ? `${rotulo}: reseta ${reset}` : undefined}>
      <span>{rotulo}</span>
      <span role="progressbar" aria-label={`${rotulo} consumido`} aria-valuenow={Math.round(usado)} aria-valuemin={0} aria-valuemax={100}
        className="w-uso-barra">
        <span className={`w-uso-preenchido ${tom.bar}`} style={{ width: `${usado}%` }} />
      </span>
      <span className={`w-uso-pct ${tom.text}`}>{janela.used_percent.toLocaleString("pt-BR")}%</span>
    </span>
  );
}

/**
 * Uso de cada conta, como o rodapé do app do PC: janela de 5 h e semanal, com as mesmas cores (verde
 * até 60%, amarelo até 85%, vermelho acima). A conta da conversa aberta vem destacada.
 */
export function UsoDasContas({ contaAtiva }: { contaAtiva: string | null }) {
  const [contas, setContas] = useState<Conta[]>([]);
  usePoll(async () => {
    const lista = await request<Conta[]>("/contas");
    setContas(Array.isArray(lista) ? lista : []);
  }, () => undefined, [], INTERVALO_MS);

  if (contas.length === 0) return null;
  return (
    <>
      <h2 className="w-rotulo">Uso das contas</h2>
      <ul className="w-uso">
        {contas.map((conta) => (
          <li key={conta.id} className={`w-uso-conta${conta.id === contaAtiva ? " w-uso-conta-ativa" : ""}`}>
            <span className="w-uso-nome">
              <AgentIcon provider={conta.provider} size={12} className="shrink-0" />
              <span className="w-atencao-texto">{conta.name}</span>
              <span className="w-uso-agente">{AGENTE[conta.provider] ?? conta.provider}</span>
            </span>
            {conta.usage && conta.usage.status !== "unavailable" ? (
              <>
                <Janela rotulo="5H" janela={conta.usage.primary} />
                <Janela rotulo="Semana" janela={conta.usage.secondary} />
              </>
            ) : (
              <span className="w-uso-vazio">{conta.usage?.reason ?? "Sem leitura de uso ainda"}</span>
            )}
          </li>
        ))}
      </ul>
    </>
  );
}
