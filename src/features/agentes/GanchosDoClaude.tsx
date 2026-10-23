import { useEffect, useState } from "react";
import { request } from "../../mobile/api";
import { AgentIcon } from "../../components/ui/AgentIcon";

/**
 * Fork (Onda 2, itens 1–3): eventos do Claude Code (ganchos HTTP só nas sessões do app) — liga/desliga,
 * quanto tempo o Medza segura um pedido de permissão antes de devolver para o terminal, e "Ler uso pela
 * statusline" por conta. Vale para as sessões que abrirem (ou reabrirem) depois. Os ganchos do próprio
 * usuário continuam valendo; com `disableAllHooks` nas configurações dele, o app não liga nada.
 */
interface ContaDosGanchos { id: string; nome: string; statusline: boolean }
/** Fork (harness, A1): `eventos` (padrão ligado) só observa; `ligado` = segurar o pedido de permissão pelo app. */
interface AjustesDosGanchos { eventos?: boolean; ligado: boolean; espera_s: number; esperas: number[]; contas: ContaDosGanchos[] }

const ROTA = "/ganchos/ajustes";

export function GanchosDoClaude() {
  const [ajustes, setAjustes] = useState<AjustesDosGanchos | null>(null);
  const [erro, setErro] = useState<string | null>(null);

  useEffect(() => {
    let vivo = true;
    request<AjustesDosGanchos>(ROTA)
      .then((dados) => { if (vivo) setAjustes(dados); })
      .catch(() => { if (vivo) setErro("Engine antigo: reinicie o app para configurar."); });
    return () => { vivo = false; };
  }, []);

  async function gravar(mudanca: Record<string, unknown>) {
    setErro(null);
    try {
      setAjustes(await request<AjustesDosGanchos>(ROTA, {
        method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(mudanca),
      }));
    } catch (motivo) {
      setErro(motivo instanceof Error ? motivo.message : String(motivo));
    }
  }

  return (
    <section className="glass mt-4 rounded-none border border-border-subtle p-4" aria-labelledby="ganchos-claude">
      <h3 id="ganchos-claude" className="flex items-center gap-2 text-xs font-semibold text-text-primary">
        <AgentIcon provider="claude" size={14} />Claude Code: eventos e permissões pelo app
      </h3>
      <p className="mt-1 text-[11px] text-text-secondary">
        O app recebe os eventos da sessão (início e fim do turno, erro, limite de uso) direto do Claude Code, em vez de só ler o
        terminal. Vale para as conversas que abrirem ou reiniciarem depois. Sem eventos (versão antiga, pasta ainda sem confiança),
        tudo segue pela tela como antes. Responder permissões pelo app é opcional: desligado, o pedido aparece como sempre.
      </p>
      <label className="mt-3 flex cursor-pointer items-center gap-2 text-xs">
        <input type="checkbox" checked={ajustes?.eventos ?? true} disabled={!ajustes} onChange={(e) => void gravar({ eventos: e.target.checked })} />
        <span className="font-semibold text-text-primary">Receber eventos do Claude Code</span>
      </label>
      <label className="mt-2 flex cursor-pointer items-center gap-2 text-xs">
        <input type="checkbox" checked={ajustes?.ligado ?? false} disabled={!ajustes || ajustes.eventos === false}
          onChange={(e) => void gravar({ ligado: e.target.checked })} />
        <span className="text-text-primary">Responder pedidos de permissão pelo app</span>
      </label>
      <div className="mt-3 flex flex-wrap items-center gap-2 text-xs" role="radiogroup" aria-label="Quanto esperar sua resposta">
        <span className="text-text-secondary">Esperar sua resposta a um pedido por</span>
        {(ajustes?.esperas ?? [120, 300, 600]).map((segundos) => (
          <label key={segundos} className="flex cursor-pointer items-center gap-1">
            <input type="radio" name="ganchos-espera" checked={ajustes?.espera_s === segundos} disabled={!ajustes?.ligado || ajustes.eventos === false}
              onChange={() => void gravar({ espera_s: segundos })} />
            {segundos / 60} min
          </label>
        ))}
        <span className="text-[11px] text-text-muted">Depois disso o pedido vai para o terminal.</span>
      </div>
      {ajustes && ajustes.contas.length > 0 && (
        <div className="mt-4 border-t border-border-subtle pt-3">
          <p className="text-xs font-semibold text-text-primary">Ler uso pela statusline</p>
          <p className="mt-1 text-[11px] text-text-secondary">
            Contexto, custo estimado e limites de 5 h e da semana vêm da statusline do Claude Code. Com ela, o Claude Code esconde a
            maior parte das dicas do rodapé do terminal (como "esc to interrupt"); a sua statusline continua aparecendo. Só entra em
            conversa que já recebeu eventos, a partir da próxima abertura.
          </p>
          <div className="mt-2 flex flex-col gap-1">
            {ajustes.contas.map((conta) => (
              <label key={conta.id} className="flex cursor-pointer items-center gap-2 text-xs">
                <input type="checkbox" checked={conta.statusline} disabled={!ajustes.ligado}
                  onChange={(e) => void gravar({ statusline: { conta: conta.id, ligado: e.target.checked } })} />
                {conta.nome}
              </label>
            ))}
          </div>
        </div>
      )}
      {erro && <p role="alert" className="mt-2 text-[11px] text-danger">{erro}</p>}
    </section>
  );
}
