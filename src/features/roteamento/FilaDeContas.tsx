import { useEffect, useState } from "react";
import { Button, Card, Input, SegmentedControl, Textarea } from "../../components/ui";
import { AgentIcon } from "../../components/ui/AgentIcon";
import { nomeDaPlataforma } from "../../lib/plataformasDeAgente";
import {
  gravarFila, lerFila, moverNaFila, resumoDaCota,
  type AjustesDaConta, type ContaDaFila, type EstadoDaFila, type ModoDaFila, type Politica,
} from "./roteamentoService";
import { PorQueEstaConta } from "./PorQueEstaConta";

/**
 * Fork (3B): Configurações → Fila de contas. Ordena as contas, liga o modo fila/automático, ajusta
 * orçamento, uso extra, concorrência e validade por conta, marca pastas confidenciais e mostra o
 * "por que esta conta" que o engine calcula. Salva na hora (`routing.json` na pasta do engine).
 */
const MODOS: { value: ModoDaFila; label: string; title: string }[] = [
  { value: "fila", label: "Fila (ordem)", title: "A primeira conta da fila que passar nos filtros" },
  { value: "auto", label: "Automático (menor uso)", title: "Entre as que passam, a de menor uso; a ordem só desempata" },
];

const nomeDoProvider = (id: string) => nomeDaPlataforma(id) ?? id;

export function FilaDeContas() {
  const [estado, setEstado] = useState<EstadoDaFila | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [aberta, setAberta] = useState<string | null>(null);

  useEffect(() => {
    let vivo = true;
    lerFila()
      .then((lido) => { if (vivo) setEstado(lido); })
      .catch(() => { if (vivo) setErro("Engine antigo: reinicie o app para configurar a fila de contas."); });
    return () => { vivo = false; };
  }, []);

  async function aplicar(mudar: (politica: Politica) => Politica) {
    if (!estado) return;
    const nova = mudar(estado.politica);
    setEstado({ ...estado, politica: nova });
    setErro(null);
    try {
      setEstado(await gravarFila(nova));
    } catch (motivo) {
      setErro(motivo instanceof Error ? motivo.message : String(motivo));
      lerFila().then(setEstado).catch(() => undefined);
    }
  }

  if (!estado) {
    return (
      <Card title="Fila de contas">
        <p className="text-xs text-text-secondary" role={erro ? "alert" : undefined}>{erro ?? "Carregando…"}</p>
      </Card>
    );
  }

  const { politica, contas, agora_ms: agora } = estado;
  const naFila = politica.etapas.map((id) => ({ id, conta: contas.find((c) => c.id === id) }));
  const fora = contas.filter((conta) => !politica.etapas.includes(conta.id));
  const providersSemFila = [...new Set(contas.map((c) => c.provider))]
    .filter((provider) => !naFila.some((etapa) => etapa.conta?.provider === provider));
  const ajustesDe = (id: string): AjustesDaConta => politica.contas[id] ?? { tipo: "assinatura" };
  const mudarAjustes = (id: string, mudanca: Partial<AjustesDaConta>) =>
    void aplicar((p) => ({ ...p, contas: { ...p.contas, [id]: { ...ajustesDe(id), ...mudanca } } }));

  return (
    <div className="settings-grade">
      <Card title="Fila de contas">
        <p className="text-[11px] text-text-secondary">
          Quando você escolhe a conta “Automática” no Novo agente, ou o orquestrador pede <code>conta: "auto"</code>, o app usa esta
          fila. Contas expiradas, sem orçamento, no limite de sessões ou com o limite de uso estourado são puladas. Numa conversa
          aberta a conta nunca troca sozinha: só os fluxos (sem tela) passam para a próxima conta quando uma falha por limite ou login.
        </p>
        <SegmentedControl label="Modo" options={MODOS} value={politica.modo} onChange={(modo) => void aplicar((p) => ({ ...p, modo }))} />

        <ol className="flex flex-col gap-2" aria-label="Contas na fila">
          {naFila.map(({ id, conta }, indice) => (
            <li key={id} className="border border-border-subtle p-2">
              <div className="flex flex-wrap items-center gap-2">
                <span className="w-5 text-right text-xs font-semibold text-text-muted">{indice + 1}.</span>
                {conta ? <CabecalhoDaConta conta={conta} agora={agora} /> : (
                  <span className="flex-1 text-xs text-text-muted">{id} · conta não cadastrada (pulada)</span>
                )}
                <div className="ml-auto flex items-center gap-1">
                  <Button size="sm" variant="ghost" aria-label={`Subir ${conta?.nome ?? id}`} disabled={indice === 0}
                    onClick={() => void aplicar((p) => ({ ...p, etapas: moverNaFila(p.etapas, id, -1) }))}>↑</Button>
                  <Button size="sm" variant="ghost" aria-label={`Descer ${conta?.nome ?? id}`} disabled={indice === naFila.length - 1}
                    onClick={() => void aplicar((p) => ({ ...p, etapas: moverNaFila(p.etapas, id, 1) }))}>↓</Button>
                  {conta && (
                    <Button size="sm" variant="ghost" aria-expanded={aberta === id} onClick={() => setAberta(aberta === id ? null : id)}>
                      Ajustes
                    </Button>
                  )}
                  <Button size="sm" variant="ghost" aria-label={`Tirar ${conta?.nome ?? id} da fila`}
                    onClick={() => void aplicar((p) => ({ ...p, etapas: p.etapas.filter((e) => e !== id) }))}>Tirar</Button>
                </div>
              </div>
              {conta && aberta === id && <AjustesDaContaForm id={id} ajustes={ajustesDe(id)} onChange={(m) => mudarAjustes(id, m)} />}
            </li>
          ))}
          {naFila.length === 0 && <li className="text-xs text-text-muted">Nenhuma conta na fila.</li>}
        </ol>
        {providersSemFila.length > 0 && (
          <p className="text-[11px] text-text-muted">
            Sem conta de {providersSemFila.map(nomeDoProvider).join(", ")} na fila: para esses, a Automática usa as contas cadastradas, na ordem.
          </p>
        )}

        {fora.length > 0 && (
          <div className="border-t border-border-subtle pt-3">
            <p className="text-xs font-semibold text-text-primary">Fora da fila</p>
            <ul className="mt-2 flex flex-col gap-2" aria-label="Contas fora da fila">
              {fora.map((conta) => (
                <li key={conta.id} className="flex flex-wrap items-center gap-2">
                  <CabecalhoDaConta conta={conta} agora={agora} />
                  <Button size="sm" variant="ghost" className="ml-auto" aria-label={`Pôr ${conta.nome} na fila`}
                    onClick={() => void aplicar((p) => ({ ...p, etapas: [...p.etapas, conta.id] }))}>Pôr na fila</Button>
                </li>
              ))}
            </ul>
          </div>
        )}
        {erro && <p role="alert" className="text-xs text-danger">{erro}</p>}
      </Card>

      <Card title="Projetos e fluxos">
        <ListaDePastas
          valor={politica.projetos_confidenciais}
          onCommit={(pastas) => void aplicar((p) => ({ ...p, projetos_confidenciais: pastas }))}
        />
        <label className="flex cursor-pointer items-start gap-2 text-xs">
          <input type="checkbox" checked={politica.usar_em_fluxos}
            onChange={(e) => void aplicar((p) => ({ ...p, usar_em_fluxos: e.target.checked }))} />
          <span>
            <span className="font-semibold text-text-primary">Usar a fila nos fluxos</span>
            <span className="block text-[11px] text-text-secondary">
              Agentes de fluxo sem conta definida rodam pela fila. Com a conta “auto” no modelo de agente, a fila vale sempre.
            </span>
          </span>
        </label>
      </Card>

      <Card title="Por que esta conta">
        {Object.entries(estado.escolhas).map(([provider, escolha]) => (
          <div key={provider} className="space-y-1">
            <p className="flex items-center gap-2 text-xs font-semibold text-text-primary">
              <AgentIcon provider={provider} size={14} />
              {nomeDoProvider(provider)} → {escolha.nome ?? "nenhuma conta apta"}
            </p>
            <PorQueEstaConta escolha={escolha} />
          </div>
        ))}
        {Object.keys(estado.escolhas).length === 0 && <p className="text-xs text-text-muted">Nenhuma conta cadastrada.</p>}
      </Card>
    </div>
  );
}

function CabecalhoDaConta({ conta, agora }: { conta: ContaDaFila; agora: number }) {
  return (
    <span className="flex min-w-0 flex-1 items-center gap-2">
      <AgentIcon provider={conta.provider} size={16} className="shrink-0" />
      <span className="min-w-0">
        <span className="block truncate text-xs font-semibold text-text-primary">
          {conta.nome}
          {conta.ajustes.tipo === "api" && <span className="ml-1 font-normal text-text-muted">· API</span>}
          {conta.ajustes.plano && <span className="ml-1 font-normal text-text-muted">· {conta.ajustes.plano}</span>}
        </span>
        <span className="block truncate text-[11px] text-text-secondary">
          {resumoDaCota(conta.cota, agora)}
          {conta.sessoes > 0 && ` · ${conta.sessoes} ${conta.sessoes === 1 ? "sessão aberta" : "sessões abertas"}`}
        </span>
      </span>
    </span>
  );
}

/** Número digitado: grava ao sair do campo (ou Enter), vazio = sem valor. */
function CampoNumero({ id, rotulo, valor, onCommit, inteiro = false }: {
  id: string; rotulo: string; valor: number | null | undefined; onCommit: (valor: number | null) => void; inteiro?: boolean;
}) {
  const [texto, setTexto] = useState(valor === null || valor === undefined ? "" : String(valor));
  useEffect(() => { setTexto(valor === null || valor === undefined ? "" : String(valor)); }, [valor]);
  const commit = () => {
    const limpo = texto.trim().replace(",", ".");
    const numero = limpo === "" ? null : Number(limpo);
    if (numero !== null && (!Number.isFinite(numero) || numero < 0 || (inteiro && !Number.isInteger(numero)))) return;
    if (numero !== (valor ?? null)) onCommit(numero);
  };
  return (
    <label htmlFor={id} className="flex flex-col gap-1 text-[11px] text-text-secondary">
      {rotulo}
      <Input id={id} size="sm" className="w-28" inputMode="decimal" value={texto}
        onChange={(e) => setTexto(e.target.value)} onBlur={commit} onKeyDown={(e) => { if (e.key === "Enter") commit(); }} />
    </label>
  );
}

const dataDoCampo = (ms: number | null | undefined) => {
  if (!ms) return "";
  const data = new Date(ms);
  const doisDigitos = (n: number) => String(n).padStart(2, "0");
  return `${data.getFullYear()}-${doisDigitos(data.getMonth() + 1)}-${doisDigitos(data.getDate())}`;
};

function AjustesDaContaForm({ id, ajustes, onChange }: { id: string; ajustes: AjustesDaConta; onChange: (mudanca: Partial<AjustesDaConta>) => void }) {
  const [plano, setPlano] = useState(ajustes.plano ?? "");
  const api = ajustes.tipo === "api";
  return (
    <div className="mt-3 grid gap-3 border-t border-border-subtle pt-3 sm:grid-cols-2" aria-label="Ajustes da conta" role="group">
      <SegmentedControl label="Tipo" size="sm" value={ajustes.tipo} onChange={(tipo) => onChange({ tipo })}
        options={[{ value: "assinatura", label: "Assinatura" }, { value: "api", label: "API" }]} />
      <label htmlFor={`plano-${id}`} className="flex flex-col gap-1 text-[11px] text-text-secondary">
        Plano (só para exibir)
        <Input id={`plano-${id}`} size="sm" value={plano} maxLength={60} placeholder="Pro, Max, Team…"
          onChange={(e) => setPlano(e.target.value)} onBlur={() => { if (plano.trim() !== (ajustes.plano ?? "")) onChange({ plano: plano.trim() || null }); }} />
      </label>
      {!api && (
        <label className="flex cursor-pointer items-start gap-2 text-xs sm:col-span-2">
          <input type="checkbox" checked={Boolean(ajustes.uso_extra)} onChange={(e) => onChange({ uso_extra: e.target.checked })} />
          <span>
            <span className="text-text-primary">Aceita uso extra</span>
            <span className="block text-[11px] text-text-secondary">
              Passa do limite da janela cobrando à parte, só enquanto houver orçamento abaixo. Sem orçamento, nunca entra em uso extra.
            </span>
          </span>
        </label>
      )}
      <CampoNumero id={`orcamento-${id}`} rotulo={api ? "Orçamento (US$)" : "Orçamento do uso extra (US$)"} valor={ajustes.orcamento_usd}
        onCommit={(orcamento_usd) => onChange({ orcamento_usd })} />
      <CampoNumero id={`gasto-${id}`} rotulo="Já gasto (US$, você atualiza)" valor={ajustes.gasto_usd ?? 0}
        onCommit={(gasto) => onChange({ gasto_usd: gasto ?? 0 })} />
      <CampoNumero id={`concorrencia-${id}`} rotulo="Sessões simultâneas (vazio = sem limite)" valor={ajustes.max_concorrencia} inteiro
        onCommit={(max) => onChange({ max_concorrencia: max === null || max < 1 ? null : max })} />
      <label htmlFor={`expira-${id}`} className="flex flex-col gap-1 text-[11px] text-text-secondary">
        Expira em (assento temporário, teste)
        <Input id={`expira-${id}`} size="sm" type="date" className="w-40" value={dataDoCampo(ajustes.expira_em_ms)}
          onChange={(e) => {
            const [ano, mes, dia] = e.target.value.split("-").map(Number);
            onChange({ expira_em_ms: e.target.value ? new Date(ano, mes - 1, dia).getTime() : null });
          }} />
      </label>
      <label className="flex cursor-pointer items-start gap-2 text-xs sm:col-span-2">
        <input type="checkbox" checked={Boolean(ajustes.confidencial_ok)} onChange={(e) => onChange({ confidencial_ok: e.target.checked })} />
        <span>
          <span className="text-text-primary">Pode receber projeto confidencial</span>
          <span className="block text-[11px] text-text-secondary">Por exemplo, a conta da empresa com retenção de dados combinada.</span>
        </span>
      </label>
    </div>
  );
}

function ListaDePastas({ valor, onCommit }: { valor: string[]; onCommit: (pastas: string[]) => void }) {
  const [texto, setTexto] = useState(valor.join("\n"));
  useEffect(() => { setTexto(valor.join("\n")); }, [valor]);
  const commit = () => {
    const pastas = texto.split("\n").map((linha) => linha.trim()).filter(Boolean);
    if (pastas.join("\n") !== valor.join("\n")) onCommit(pastas);
  };
  return (
    <label htmlFor="pastas-confidenciais" className="flex flex-col gap-1 text-xs">
      <span className="font-semibold text-text-primary">Projetos confidenciais</span>
      <span className="text-[11px] text-text-secondary">
        Uma pasta por linha (vale para tudo dentro dela). Agente aberto ali só usa conta marcada como “pode receber projeto confidencial”.
      </span>
      <Textarea id="pastas-confidenciais" size="sm" rows={3} className="font-mono" value={texto}
        placeholder="/Users/voce/empresa" onChange={(e) => setTexto(e.target.value)} onBlur={commit} />
    </label>
  );
}
