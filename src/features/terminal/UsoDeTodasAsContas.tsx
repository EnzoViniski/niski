import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { ChartIcon, ReloadIcon } from "../../components/ui/PixelIcon";
import { AgentIcon } from "../../components/ui/AgentIcon";
import { resetShort, usageTone } from "../../lib/uso";
import type { Usage, UsageWindow } from "./AccountUsage";
import { listProfiles, type Profile } from "./terminalService";

const AGENTE: Record<string, string> = { claude: "Claude", codex: "Codex" };

function Janela({ rotulo, janela }: { rotulo: string; janela: UsageWindow | null }) {
  const usado = janela ? Math.min(100, Math.max(0, janela.used_percent)) : 0;
  const tom = janela ? usageTone(janela.used_percent) : null;
  const reset = resetShort(janela);
  return (
    <span className="grid grid-cols-[3.5rem_1fr_2.75rem] items-center gap-2" title={reset ? `${rotulo}: reseta ${reset}` : undefined}>
      <span className="text-text-secondary">{rotulo}</span>
      <span role="progressbar" aria-label={`${rotulo} consumido`} aria-valuenow={Math.round(usado)} aria-valuemin={0} aria-valuemax={100}
        className="relative block h-1.5 bg-overlay/[0.16]">
        {tom && <span className={`absolute inset-y-0 left-0 ${tom.bar}`} style={{ width: `${usado}%` }} />}
      </span>
      <span className={`text-right font-bold ${tom?.text ?? "text-text-muted"}`}>
        {janela ? `${janela.used_percent.toLocaleString("pt-BR")}%` : "—"}
      </span>
    </span>
  );
}

/**
 * Botão do rodapé, à esquerda do agente em foco, que abre (para cima) o uso de todas as contas de
 * Claude e Codex. Abrir só lê o cache do engine; o botão de cada conta é que consulta o `/usage`.
 */
export function UsoDeTodasAsContas({ engineOnline, contaAtiva }: { engineOnline: boolean; contaAtiva: string | null }) {
  const [aberto, setAberto] = useState(false);
  const [contas, setContas] = useState<Profile[]>([]);
  const [usos, setUsos] = useState<Record<string, Usage>>({});
  const [erros, setErros] = useState<Record<string, string>>({});
  const [atualizando, setAtualizando] = useState<Set<string>>(new Set());
  const [posicao, setPosicao] = useState<{ left: number; bottom: number } | null>(null);
  const botao = useRef<HTMLButtonElement>(null);
  const painel = useRef<HTMLDivElement>(null);

  // Cada abertura relê a lista e o cache: conta nova ou um /usage feito em outro lugar já aparecem.
  useEffect(() => {
    if (!aberto || !engineOnline) return;
    let cancelado = false;
    void listProfiles().then((lista) => {
      if (cancelado) return;
      const comUso = lista.filter((conta) => conta.provider === "claude" || conta.provider === "codex");
      setContas(comUso);
      for (const conta of comUso) {
        invoke<Usage>("account_usage", { profileId: conta.id, refresh: false })
          .then((uso) => { if (!cancelado) setUsos((atual) => ({ ...atual, [conta.id]: uso })); })
          .catch(() => undefined);
      }
    }).catch(() => undefined);
    return () => { cancelado = true; };
  }, [aberto, engineOnline]);

  // O rodapé corta o que passa dele (`overflow-hidden`): o painel vai para o `body`, ancorado no botão.
  useLayoutEffect(() => {
    if (!aberto) return;
    const ancorar = () => {
      const caixa = botao.current?.getBoundingClientRect();
      if (!caixa) return;
      const largura = 320;
      setPosicao({
        left: Math.max(8, Math.min(caixa.left, window.innerWidth - largura - 8)),
        bottom: window.innerHeight - caixa.top + 6,
      });
    };
    ancorar();
    window.addEventListener("resize", ancorar);
    return () => window.removeEventListener("resize", ancorar);
  }, [aberto]);

  useEffect(() => {
    if (!aberto) return;
    const fora = (evento: MouseEvent) => {
      const alvo = evento.target as Node;
      if (!painel.current?.contains(alvo) && !botao.current?.contains(alvo)) setAberto(false);
    };
    const esc = (evento: KeyboardEvent) => { if (evento.key === "Escape") setAberto(false); };
    document.addEventListener("mousedown", fora);
    document.addEventListener("keydown", esc);
    return () => { document.removeEventListener("mousedown", fora); document.removeEventListener("keydown", esc); };
  }, [aberto]);

  async function atualizar(id: string) {
    setAtualizando((atual) => new Set(atual).add(id));
    setErros(({ [id]: _, ...resto }) => resto);
    try {
      const uso = await invoke<Usage>("account_usage", { profileId: id, refresh: true });
      setUsos((atual) => ({ ...atual, [id]: uso }));
    } catch (motivo) {
      setErros((atual) => ({ ...atual, [id]: String(motivo) }));
    } finally {
      setAtualizando((atual) => { const novo = new Set(atual); novo.delete(id); return novo; });
    }
  }

  return (
    <>
      <button ref={botao} type="button" aria-label="Uso de todas as contas" aria-expanded={aberto} aria-haspopup="dialog"
        title="Uso de todas as contas" disabled={!engineOnline} onClick={() => setAberto((atual) => !atual)}
        className={`shrink-0 disabled:opacity-50 ${aberto ? "text-accent" : "text-text-secondary hover:text-accent"}`}>
        <ChartIcon aria-hidden className="h-3.5 w-3.5" />
      </button>
      {aberto && posicao && createPortal(
        <div ref={painel} role="dialog" aria-label="Uso das contas" style={{ left: posicao.left, bottom: posicao.bottom }}
          className="fixed z-[200] flex max-h-[70vh] w-80 flex-col overflow-y-auto border-2 border-border-subtle bg-bg-elevated p-2 text-[11px] text-text-primary shadow-[0_12px_28px_rgb(0_0_0/0.35)]">
          <p className="px-1 pb-1.5 text-[10px] font-bold uppercase tracking-wider text-text-muted">Uso das contas</p>
          {contas.length === 0 && <p className="px-1 py-2 text-text-secondary">Nenhuma conta de Claude ou Codex.</p>}
          <ul className="flex flex-col gap-1.5">
            {contas.map((conta) => {
              const uso = usos[conta.id];
              const girando = atualizando.has(conta.id);
              const motivo = erros[conta.id] ?? (uso && uso.status === "unavailable" ? uso.reason : null);
              return (
                <li key={conta.id}
                  className={`flex flex-col gap-1 border px-2 py-1.5 ${conta.id === contaAtiva ? "border-accent/60 bg-accent/[0.08]" : "border-border-subtle"}`}>
                  <span className="flex min-w-0 items-center gap-1.5">
                    <AgentIcon provider={conta.provider} size={12} className="shrink-0" />
                    <span className="min-w-0 flex-1 truncate font-bold" title={conta.email ?? undefined}>{conta.name}</span>
                    <span className="text-[10px] uppercase text-text-muted">{AGENTE[conta.provider] ?? conta.provider}</span>
                    <button type="button" aria-label={`Atualizar uso de ${conta.name} (${AGENTE[conta.provider]})`} title="Consultar o uso agora"
                      disabled={girando} onClick={() => void atualizar(conta.id)}
                      className="shrink-0 text-text-secondary hover:text-accent disabled:opacity-50">
                      <ReloadIcon aria-hidden className={`h-3.5 w-3.5${girando ? " animate-spin" : ""}`} />
                    </button>
                  </span>
                  {uso && uso.status !== "unavailable" ? (
                    <>
                      <Janela rotulo="5H" janela={uso.primary} />
                      <Janela rotulo="Semana" janela={uso.secondary} />
                    </>
                  ) : !motivo && <span className="text-text-muted">{uso ? "Sem leitura de uso ainda" : "Lendo…"}</span>}
                  {motivo && <span role="alert" className="text-warning">{motivo}</span>}
                </li>
              );
            })}
          </ul>
        </div>,
        document.body,
      )}
    </>
  );
}
