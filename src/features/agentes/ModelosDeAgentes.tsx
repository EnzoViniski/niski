import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { Button, Card, Field, Input, Select, Textarea } from "../../components/ui";
import { AgentIcon } from "../../components/ui/AgentIcon";
import { CopyIcon, EditIcon, FolderIcon, ReloadIcon, RobotIcon, SparkleIcon, TrashIcon } from "../../components/ui/PixelIcon";
import { listProfiles, type Profile } from "../terminal/terminalService";
import {
  apagarAgente, criarComIa, FERRAMENTAS_COMUNS, instalarNoClaudeCode, listarAgentes, listarModelos, modeloVazio, nomeDeArquivo,
  restaurarKit, ROTULO_ESFORCO, SAIDAS, salvarAgente, type AgenteSalvo, type CliDoAgente, type Escopo, type ModeloDaCli, type ModeloDeAgente,
} from "./agentesService";
import { ManualDeAgentes } from "./ManualDeAgentes";

const CLIS: { value: CliDoAgente; label: string }[] = [
  { value: "claude", label: "Claude" },
  { value: "codex", label: "Codex" },
  { value: "cursor", label: "Cursor" },
];
const erroDe = (e: unknown) => (e instanceof Error ? e.message : String(e));

interface Edicao { modelo: ModeloDeAgente; escopo: Escopo; nomeAnterior?: string; escopoAnterior?: Escopo }

/**
 * Configurações → Modelos de agentes: os `.md` globais e do projeto, em cartões. Criar com IA (uma
 * frase vira um modelo no padrão do mercado), editar, duplicar, apagar e instalar no Claude Code.
 * São as peças que os fluxos vão usar.
 */
export function ModelosDeAgentes({ projectPath }: { projectPath: string | null }) {
  const [agentes, setAgentes] = useState<AgenteSalvo[] | null>(null);
  const [perfis, setPerfis] = useState<Profile[]>([]);
  const [filtro, setFiltro] = useState<"todos" | Escopo>("todos");
  const [edicao, setEdicao] = useState<Edicao | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  const [descricao, setDescricao] = useState("");
  const [cliNovo, setCliNovo] = useState<CliDoAgente>("claude");
  const [criando, setCriando] = useState(false);
  const [salvando, setSalvando] = useState(false);
  const [apagando, setApagando] = useState<string | null>(null);
  const [modelos, setModelos] = useState<Partial<Record<CliDoAgente, ModeloDaCli[]>>>({});
  const [manual, setManual] = useState(false);
  useEffect(() => { listarModelos().then(setModelos).catch(() => undefined); }, []);

  const recarregar = useCallback(async () => {
    try { setAgentes(await listarAgentes(projectPath)); } catch (e) { setErro(erroDe(e)); setAgentes([]); }
  }, [projectPath]);
  useEffect(() => { void recarregar(); }, [recarregar]);
  useEffect(() => { listProfiles().then(setPerfis).catch(() => undefined); }, []);

  const visiveis = useMemo(() => (agentes ?? []).filter((a) => filtro === "todos" || a.escopo === filtro), [agentes, filtro]);

  async function criar() {
    setCriando(true); setErro(null); setAviso(null);
    try {
      const modelo = await criarComIa(descricao, cliNovo, projectPath);
      setEdicao({ modelo, escopo: "global" });
      setDescricao("");
      setAviso("A IA escreveu o modelo abaixo. Revise e salve.");
    } catch (e) { setErro(erroDe(e)); } finally { setCriando(false); }
  }

  async function salvar() {
    if (!edicao) return;
    setSalvando(true); setErro(null);
    try {
      const { escopo, nomeAnterior, escopoAnterior } = edicao;
      // Fork: normalize só ao salvar para preservar o hífen durante a digitação; o backend valida o nome.
      const modelo = { ...edicao.modelo, name: nomeDeArquivo(edicao.modelo.name) };
      // Mudou de escopo: grava no novo e tira do antigo, para não ficar duplicado.
      await salvarAgente(escopo, projectPath, modelo, escopoAnterior === escopo ? nomeAnterior : undefined);
      if (escopoAnterior && escopoAnterior !== escopo && nomeAnterior) await apagarAgente(escopoAnterior, projectPath, nomeAnterior);
      setEdicao(null);
      setAviso(`"${modelo.name}" salvo.`);
      await recarregar();
    } catch (e) { setErro(erroDe(e)); } finally { setSalvando(false); }
  }

  async function apagar(a: AgenteSalvo) {
    const chave = `${a.escopo}:${a.name}`;
    if (apagando !== chave) { setApagando(chave); return; }
    setApagando(null);
    try { await apagarAgente(a.escopo, projectPath, a.name); await recarregar(); } catch (e) { setErro(erroDe(e)); }
  }

  async function instalar(a: AgenteSalvo) {
    if (!projectPath) return;
    try {
      const caminho = await instalarNoClaudeCode(projectPath, a);
      setAviso(`Instalado em ${caminho}. O Claude Code deste projeto já pode usá-lo como subagente.`);
    } catch (e) { setErro(erroDe(e)); }
  }

  return (
    <div className="settings-grade">
      <Card title="Criar com IA" className="settings-largo"
        aside={<span className="text-[11px] text-text-muted">A IA escreve no padrão dos times de agentes: papel, objetivo, passos, limites e saída.</span>}>
        <Textarea value={descricao} onChange={(e) => setDescricao(e.target.value)} rows={2} maxLength={600}
          placeholder="Ex.: um agente que revisa a acessibilidade de telas React — contraste, foco e leitores de tela"
          aria-label="Descreva o agente" />
        <div className="flex flex-wrap items-center gap-2">
          <div className="w-40"><Select value={cliNovo} options={CLIS} onChange={setCliNovo} size="sm" aria-label="CLI do agente" /></div>
          <Button variant="primary" onClick={() => void criar()} disabled={criando || descricao.trim().length < 8}>
            {criando ? <><ReloadIcon aria-hidden className="h-3.5 w-3.5 animate-spin" />Escrevendo… (~40 s)</> : <><SparkleIcon aria-hidden className="h-3.5 w-3.5" />Criar com IA</>}
          </Button>
          <Button variant="ghost" onClick={() => { setAviso(null); setEdicao({ modelo: modeloVazio(), escopo: "global" }); }}>
            <EditIcon aria-hidden className="h-3.5 w-3.5" />Em branco
          </Button>
          <span className="flex-1" />
          <Button variant="ghost" size="sm" onClick={() => setManual(true)}>Como usar</Button>
          <Button variant="ghost" size="sm" title="Regrava os 7 modelos que vêm com o OMNI (os seus continuam)"
            onClick={() => void restaurarKit().then(recarregar).catch((e) => setErro(erroDe(e)))}>
            Restaurar kit do OMNI
          </Button>
        </div>
        {erro && <p role="alert" className="text-xs text-danger">{erro}</p>}
        {aviso && <p role="status" className="text-xs text-success">{aviso}</p>}
      </Card>

      {manual && <ManualDeAgentes onFechar={() => setManual(false)} />}
      {edicao && (
        <EditorDeAgente edicao={edicao} perfis={perfis} modelos={modelos} temProjeto={Boolean(projectPath)} salvando={salvando}
          onMudar={setEdicao} onSalvar={() => void salvar()} onCancelar={() => setEdicao(null)} />
      )}

      <Card title="Modelos salvos" className="settings-largo"
        aside={
          <div className="flex items-center gap-1" role="tablist" aria-label="Filtrar por escopo">
            {(["todos", "global", "projeto"] as const).map((f) => (
              <button key={f} type="button" role="tab" aria-selected={filtro === f} onClick={() => setFiltro(f)}
                className={`agentes-filtro${filtro === f ? " is-ativo" : ""}`}>
                {f === "todos" ? "Todos" : f === "global" ? "Globais" : "Do projeto"}
                <span>{(agentes ?? []).filter((a) => f === "todos" || a.escopo === f).length}</span>
              </button>
            ))}
          </div>
        }>
        {agentes === null ? <p className="text-xs text-text-muted">Lendo os modelos…</p>
          : visiveis.length === 0 ? (
            <p className="text-xs text-text-muted">
              {filtro === "projeto" ? (projectPath ? "Este projeto ainda não tem modelos próprios. Salve um com o escopo “Projeto”." : "Abra um projeto para ver os modelos dele.") : "Nenhum modelo ainda."}
            </p>
          ) : (
            <div className="agentes-grade">
              {visiveis.map((a) => {
                const chave = `${a.escopo}:${a.name}`;
                return (
                  <article key={chave} className="agente-cartao">
                    <header className="agente-cartao-topo">
                      <span className="agente-cartao-icone"><AgentIcon provider={a.cli} size={20} /></span>
                      <div className="min-w-0 flex-1">
                        <h3 className="agente-cartao-nome">{a.name}</h3>
                        <p className="agente-cartao-sub">{CLIS.find((c) => c.value === a.cli)?.label}{a.modelo ? ` · ${a.modelo}` : ""}{a.esforco ? ` · esforço ${(ROTULO_ESFORCO[a.esforco] ?? a.esforco).toLowerCase()}` : ""}</p>
                      </div>
                      <span className={`agente-escopo is-${a.escopo}`}>{a.escopo === "global" ? "Global" : "Projeto"}</span>
                    </header>
                    <p className="agente-cartao-desc">{a.description}</p>
                    <div className="agente-chips">
                      <span className="agente-chip is-saida">{SAIDAS.find((s) => s.value === a.saida)?.label ?? a.saida}</span>
                      {a.ferramentas.map((f) => <span key={f} className="agente-chip">{f}</span>)}
                    </div>
                    <footer className="agente-acoes">
                      <button type="button" onClick={() => { setAviso(null); setEdicao({ modelo: { ...a }, escopo: a.escopo, nomeAnterior: a.name, escopoAnterior: a.escopo }); }}>
                        <EditIcon aria-hidden className="h-3.5 w-3.5" />Editar
                      </button>
                      <button type="button" onClick={() => { setAviso(null); setEdicao({ modelo: { ...a, name: nomeDeArquivo(`${a.name}-copia`) }, escopo: a.escopo }); }}>
                        <CopyIcon aria-hidden className="h-3.5 w-3.5" />Duplicar
                      </button>
                      {a.cli === "claude" && projectPath && (
                        <button type="button" title="Grava em .claude/agents do projeto: o Claude Code passa a usá-lo como subagente" onClick={() => void instalar(a)}>
                          <RobotIcon aria-hidden className="h-3.5 w-3.5" />No Claude Code
                        </button>
                      )}
                      <button type="button" title={a.caminho} onClick={() => void revealItemInDir(a.caminho).catch(() => undefined)}>
                        <FolderIcon aria-hidden className="h-3.5 w-3.5" />Arquivo
                      </button>
                      <span className="flex-1" />
                      <button type="button" className={`is-perigo${apagando === chave ? " is-confirmar" : ""}`} onClick={() => void apagar(a)}
                        onBlur={() => setApagando((atual) => (atual === chave ? null : atual))}>
                        <TrashIcon aria-hidden className="h-3.5 w-3.5" />{apagando === chave ? "Confirmar" : "Apagar"}
                      </button>
                    </footer>
                  </article>
                );
              })}
            </div>
          )}
      </Card>
    </div>
  );
}

/** Modelo e esforço da CLI escolhida. A lista vem da CLI (Codex) ou é a do Claude; um valor que não
 *  está na lista (escrito à mão no arquivo) continua aparecendo como opção. */
export function SeletorDeModelo({ cli, modelos, modelo, esforco, onMudar, rotuloPadrao = "Padrão da CLI" }: {
  cli: CliDoAgente; modelos: Partial<Record<CliDoAgente, ModeloDaCli[]>>; modelo: string; esforco: string;
  onMudar: (parte: { modelo?: string; esforco?: string }) => void; rotuloPadrao?: string;
}) {
  const lista = modelos[cli] ?? [];
  const atual = lista.find((m) => m.id === modelo);
  const esforcos = atual?.esforcos ?? lista[0]?.esforcos ?? [];
  const opcoesModelo = [{ value: "", label: rotuloPadrao }, ...lista.map((m) => ({ value: m.id, label: m.label })),
    ...(modelo && !atual ? [{ value: modelo, label: modelo }] : [])];
  const opcoesEsforco = [{ value: "", label: rotuloPadrao }, ...esforcos.map((e) => ({ value: e, label: ROTULO_ESFORCO[e] ?? e })),
    ...(esforco && !esforcos.includes(esforco) ? [{ value: esforco, label: esforco }] : [])];
  return (
    <>
      <Field label="Modelo" htmlFor={`ag-modelo-${cli}`} description={lista.length ? "Opus/Fable para raciocínio, Sonnet para o dia a dia, Haiku para tarefas curtas." : "Esta CLI não informa a lista de modelos."}>
        <Select id={`ag-modelo-${cli}`} value={modelo} options={opcoesModelo} onChange={(v) => onMudar({ modelo: v })} />
      </Field>
      <Field label="Esforço" htmlFor={`ag-esforco-${cli}`} description="Quanto o modelo pensa antes de agir: mais esforço, mais qualidade e mais tokens.">
        <Select id={`ag-esforco-${cli}`} value={esforco} options={opcoesEsforco} onChange={(v) => onMudar({ esforco: v })} disabled={esforcos.length === 0 && !esforco} />
      </Field>
    </>
  );
}

function EditorDeAgente({ edicao, perfis, modelos, temProjeto, salvando, onMudar, onSalvar, onCancelar }: {
  edicao: Edicao; perfis: Profile[]; modelos: Partial<Record<CliDoAgente, ModeloDaCli[]>>; temProjeto: boolean; salvando: boolean;
  onMudar: (e: Edicao) => void; onSalvar: () => void; onCancelar: () => void;
}) {
  const { modelo, escopo } = edicao;
  const mudar = (parte: Partial<ModeloDeAgente>) => onMudar({ ...edicao, modelo: { ...modelo, ...parte } });
  const contas = perfis.filter((p) => p.provider === modelo.cli);
  const [ferramentaNova, setFerramentaNova] = useState("");
  const alternar = (f: string) => mudar({ ferramentas: modelo.ferramentas.includes(f) ? modelo.ferramentas.filter((x) => x !== f) : [...modelo.ferramentas, f] });
  const extras = modelo.ferramentas.filter((f) => !FERRAMENTAS_COMUNS.includes(f));
  // Abriu (ou trocou de modelo): leva o editor para a vista — ele nasce acima da lista longa.
  const raiz = useRef<HTMLDivElement>(null);
  useEffect(() => { raiz.current?.scrollIntoView?.({ behavior: "smooth", block: "start" }); }, [edicao.nomeAnterior, edicao.modelo.name === ""]);

  return (
    <div ref={raiz} className="settings-largo">
    <Card title={edicao.nomeAnterior ? `Editando “${edicao.nomeAnterior}”` : "Novo modelo"} className="settings-largo agente-editor"
      aside={
        <div className="flex gap-2">
          <Button variant="ghost" onClick={onCancelar} disabled={salvando}>Cancelar</Button>
          <Button variant="primary" onClick={onSalvar} disabled={salvando}>{salvando ? "Salvando…" : "Salvar"}</Button>
        </div>
      }>
      <div className="agente-editor-grade">
        <Field label="Nome" htmlFor="ag-nome" description="Vira o nome do arquivo: minúsculas, números e hífen.">
          <Input id="ag-nome" value={modelo.name} onChange={(e) => mudar({ name: e.target.value })} placeholder="revisor-a11y" />
        </Field>
        <Field label="Onde salvar" htmlFor="ag-escopo" description={escopo === "global" ? "Vale em todo projeto (~/.omni/agents)." : "Só neste projeto (.omni/agents, vai no git)."}>
          <Select id="ag-escopo" value={escopo} onChange={(v) => onMudar({ ...edicao, escopo: v })}
            options={[{ value: "global", label: "Global" }, { value: "projeto", label: "Projeto", disabled: !temProjeto }]} />
        </Field>
        <div className="agente-editor-largo">
          <Field label="Descrição" htmlFor="ag-desc" description="Uma frase: o que faz e quando usar. O orquestrador lê isto para escolher quem chamar.">
            <Input id="ag-desc" value={modelo.description} onChange={(e) => mudar({ description: e.target.value })} maxLength={240} />
          </Field>
        </div>
        <Field label="CLI" htmlFor="ag-cli">
          <Select id="ag-cli" value={modelo.cli} options={CLIS} onChange={(v) => mudar({ cli: v, conta: "", modelo: "", esforco: "" })} />
        </Field>
        <SeletorDeModelo cli={modelo.cli} modelos={modelos} modelo={modelo.modelo} esforco={modelo.esforco}
          onMudar={(parte) => mudar(parte)} />
        <Field label="Conta" htmlFor="ag-conta" description="Qualquer = o fluxo escolhe uma com uso disponível.">
          <Select id="ag-conta" value={modelo.conta} onChange={(v) => mudar({ conta: v })}
            options={[{ value: "", label: "Qualquer" }, ...contas.map((p) => ({ value: p.id, label: p.name }))]} />
        </Field>
        <Field label="Saída" htmlFor="ag-saida" description={SAIDAS.find((s) => s.value === modelo.saida)?.dica}>
          <Select id="ag-saida" value={modelo.saida} onChange={(v) => mudar({ saida: v })} options={SAIDAS.map(({ value, label }) => ({ value, label }))} />
        </Field>
        <div className="agente-editor-largo">
          <Field label="Ferramentas" description="Só o necessário: quem só analisa não precisa de Edit nem Write.">
            <div className="agente-chips">
              {[...FERRAMENTAS_COMUNS, ...extras].map((f) => (
                <button key={f} type="button" aria-pressed={modelo.ferramentas.includes(f)} onClick={() => alternar(f)} className="agente-chip is-botao">{f}</button>
              ))}
              <input value={ferramentaNova} onChange={(e) => setFerramentaNova(e.target.value)} placeholder="+ outra (ex.: mcp__github)"
                aria-label="Adicionar ferramenta" className="agente-chip-input"
                onKeyDown={(e) => {
                  if (e.key !== "Enter" || !ferramentaNova.trim()) return;
                  e.preventDefault();
                  if (!modelo.ferramentas.includes(ferramentaNova.trim())) mudar({ ferramentas: [...modelo.ferramentas, ferramentaNova.trim()] });
                  setFerramentaNova("");
                }} />
            </div>
          </Field>
        </div>
        <div className="agente-editor-largo">
          <Field label="Instruções" htmlFor="ag-inst" description="O que o agente lê antes de tudo: papel, objetivo, como trabalhar, limites e formato da saída.">
            <Textarea id="ag-inst" value={modelo.instrucoes} onChange={(e) => mudar({ instrucoes: e.target.value })} rows={14} className="font-mono text-xs" />
          </Field>
        </div>
      </div>
    </Card>
    </div>
  );
}
