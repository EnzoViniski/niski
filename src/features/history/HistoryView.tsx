import ReactMarkdown from "react-markdown";
import rehypeSanitize from "rehype-sanitize";
import remarkGfm from "remark-gfm";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Button, Input, SegmentedControl, Select } from "../../components/ui";
import { AgentIcon } from "../../components/ui/AgentIcon";
import { ChatIcon, ClockIcon, CloseIcon, FolderIcon, PlayIcon, ReloadIcon } from "../../components/ui/PixelIcon";
import { shortcutLabel } from "../../lib/shortcuts";
import { baseName } from "../files/filesService";
import { EditableLabel } from "../../components/ui/EditableLabel";
import { listConversations, listProfiles, renameConversation, type Conversation, type Profile } from "../terminal/terminalService";
import {
  filterHistory,
  listHistory,
  readTranscript,
  type HistoryEntry,
  type HistoryProvider,
  type HistoryTranscript,
} from "./historyService";
// Fork (retomar-sempre): erro de retomada com saídas (outra pasta, conversa nova com o histórico).
import { RetomadaError, type AcaoDeRetomada, type OpcoesDeRetomada } from "../retomada/retomadaService";
import { NOME_CURTO } from "../../lib/identidade";

const PROVIDERS = [
  { value: "claude" as const, label: "Claude", icon: <AgentIcon provider="claude" size={14} /> },
  { value: "codex" as const, label: "Codex", icon: <AgentIcon provider="codex" size={14} /> },
];

/** Lista longa (centenas de conversas) renderiza aos poucos; a busca é quem acha as antigas. */
const PAGE = 100;

function formatDate(ms: number | null) {
  return ms ? new Date(ms).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" }) : "—";
}

/** "agora", "há 5 min", "ontem", "12/10" — a lista lê melhor com tempo relativo. */
function quando(ms: number | null, agora = Date.now()) {
  if (!ms) return "—";
  const minutos = Math.floor(Math.max(0, agora - ms) / 60_000);
  if (minutos < 1) return "agora";
  if (minutos < 60) return `há ${minutos} min`;
  if (minutos < 24 * 60) return `há ${Math.floor(minutos / 60)} h`;
  if (minutos < 48 * 60) return "ontem";
  if (minutos < 7 * 24 * 60) return `há ${Math.floor(minutos / (24 * 60))} d`;
  return new Date(ms).toLocaleDateString("pt-BR", { day: "2-digit", month: "2-digit" });
}

/** Contas cadastradas, para escolher em qual a conversa reabre. */
function useContas() {
  const [contas, setContas] = useState<Profile[]>([]);
  useEffect(() => {
    void listProfiles().then(setContas).catch(() => setContas([]));
  }, []);
  return contas;
}

function errorText(reason: unknown) {
  return reason instanceof Error ? reason.message : String(reason);
}

/** Conversas do índice do OMNI, por id de sessão do provider: é o que liga uma entrada do Histórico
 *  (que vem do transcript da CLI) à conversa renomeável. Conversa aberta fora do OMNI não tem par. */
function useConversasPorSessao() {
  const [porSessao, setPorSessao] = useState<Record<string, Conversation>>({});
  useEffect(() => {
    void listConversations()
      .then((lista) => {
        const mapa: Record<string, Conversation> = {};
        for (const conversa of lista) {
          for (const segmento of conversa.segments) {
            if (segmento.external_session_id) mapa[segmento.external_session_id] = conversa;
          }
        }
        setPorSessao(mapa);
      })
      .catch(() => setPorSessao({}));
  }, []);
  return porSessao;
}

interface HistoryViewProps {
  /** Reabre a conversa no CLI dentro de um terminal do projeto dela, na conta escolhida (a da
   *  conversa por padrão; outra conta recebe uma cópia do transcript antes). */
  onResume: (entry: HistoryEntry, profileId: string, opcoes?: OpcoesDeRetomada) => Promise<void> | void;
  /** Trava o provider e a pasta — usado pelo "Retomar conversa" do Novo agente, que só pode
   *  reabrir conversas daquela CLI naquele projeto. */
  lock?: { provider: HistoryProvider; cwd: string };
  /** Botão extra no cabeçalho (ex.: voltar ao seletor de CLI). */
  actions?: ReactNode;
  /** Tela cheia: volta para onde estava, igual às Configurações. Ausente quando embutido. */
  onClose?: () => void;
}

/** Conversas gravadas pelo Claude Code e pelo Codex em todas as contas cadastradas, com o OMNI
 *  aberto ou não. Lê direto dos transcripts do CLI — nada é copiado. */
export function HistoryView({ onResume, lock, actions, onClose }: HistoryViewProps) {
  const [provider, setProvider] = useState<HistoryProvider>(lock?.provider ?? "claude");
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);
  const [reload, setReload] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [cwd, setCwd] = useState(lock?.cwd ?? "");
  const [profileId, setProfileId] = useState("");
  const [limit, setLimit] = useState(PAGE);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const conversasPorSessao = useConversasPorSessao();
  const contas = useContas();

  useEffect(() => {
    let cancelled = false;
    setEntries(null);
    setError(null);
    listHistory(provider)
      .then((list) => {
        if (!cancelled) setEntries(list);
      })
      .catch((reason) => {
        if (!cancelled) {
          setEntries([]);
          setError(errorText(reason));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [provider, reload]);

  // Trocar de provider zera filtros que só fazem sentido no outro (pasta e conta).
  function changeProvider(next: HistoryProvider) {
    setProvider(next);
    setCwd("");
    setProfileId("");
    setSelectedPath(null);
    setLimit(PAGE);
  }

  const projectOptions = useMemo(() => {
    const byKey = new Map<string, string>();
    for (const entry of entries ?? []) {
      if (entry.cwd && !byKey.has(entry.cwd.toLowerCase())) byKey.set(entry.cwd.toLowerCase(), entry.cwd);
    }
    const paths = [...byKey.values()].sort((a, b) => baseName(a).localeCompare(baseName(b)));
    return [{ value: "", label: "Todos os projetos" }, ...paths.map((path) => ({ value: path, label: baseName(path), hint: path }))];
  }, [entries]);

  const profileOptions = useMemo(() => {
    const names = new Map((entries ?? []).map((entry) => [entry.profile_id, entry.profile_name]));
    return [{ value: "", label: "Todas as contas" }, ...[...names].map(([value, label]) => ({ value, label }))];
  }, [entries]);

  const filtered = useMemo(() => filterHistory(entries ?? [], { query, cwd, profileId }), [entries, query, cwd, profileId]);
  const selected = filtered.find((entry) => entry.path === selectedPath) ?? null;

  return (
    <div className="historico">
      <header className="historico-topo glass">
        <span className="historico-topo-icone" aria-hidden>
          <ChatIcon className="h-5 w-5" />
        </span>
        <div className="min-w-0 flex-1">
          <h1 className="pixel-text text-text-primary text-sm">{lock ? "Retomar conversa" : "Histórico de conversas"}</h1>
          <p className="text-text-secondary text-[11px]">
            {lock
              ? `Conversas do ${lock.provider === "claude" ? "Claude" : "Codex"} neste projeto. Escolha uma, veja as mensagens e reabra na conta que quiser.`
              : `Tudo que o Claude Code e o Codex gravaram, em todas as contas — com o ${NOME_CURTO} aberto ou não.`}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {!lock && <SegmentedControl options={PROVIDERS} value={provider} onChange={changeProvider} grow={false} />}
          <Button variant="ghost" size="sm" onClick={() => setReload((value) => value + 1)} disabled={entries === null}
            aria-label="Atualizar" title="Ler os transcripts de novo">
            <ReloadIcon className={`h-3.5 w-3.5${entries === null ? " animate-spin" : ""}`} aria-hidden="true" />
          </Button>
          {actions}
          {onClose && (
            <Button
              size="sm"
              variant="ghost"
              onClick={onClose}
              aria-label="Fechar histórico"
              title={`Fechar histórico (${shortcutLabel("closeSettings")})`}
            >
              <CloseIcon className="h-3.5 w-3.5" aria-hidden="true" />
              Fechar
            </Button>
          )}
        </div>
      </header>

      <div className="historico-filtros glass">
        <Input
          size="sm"
          type="search"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setLimit(PAGE);
          }}
          placeholder="Pesquisar por título, prompt ou pasta"
          aria-label="Pesquisar conversas"
          className="min-w-0 flex-[2_1_14rem]"
        />
        {!lock && <Select
          size="sm"
          value={cwd}
          options={projectOptions}
          onChange={(value) => {
            setCwd(value);
            setLimit(PAGE);
          }}
          searchable={projectOptions.length > 15}
          searchPlaceholder="Pesquisar projeto…"
          className="min-w-0 flex-[1_1_10rem]"
        />}
        {profileOptions.length > 2 && (
          <Select size="sm" value={profileId} options={profileOptions} onChange={setProfileId} className="min-w-0 flex-[1_1_8rem]" />
        )}
      </div>

      {error && (
        <p role="alert" className="border-l-2 border-danger pl-3 text-xs text-danger">
          {error}
        </p>
      )}

      <div className="historico-corpo">
        <section aria-label="Conversas" className="historico-lista glass">
          <p className="historico-contagem">
            {entries === null ? "Lendo transcripts…" : `${filtered.length} de ${entries.length} conversas`}
          </p>
          <ul className="min-h-0 flex-1 space-y-1 overflow-y-auto p-1">
            {entries === null && Array.from({ length: 6 }, (_, i) => <li key={i} className="historico-esqueleto" aria-hidden />)}
            {/* Mesmo cartão das conversas da lateral (`Sidebar` → `SessionLabel`). */}
            {filtered.slice(0, limit).map((entry) => {
              const ativa = entry.path === selectedPath;
              return (
                <li key={entry.path}
                  className={`lateral-item border-l-2 ring-1 ring-inset transition-colors ${
                    ativa ? "lateral-item-ativo border-accent ring-accent/50" : "border-transparent ring-border-subtle/40 hover:ring-border-subtle"
                  }`}>
                  <button
                    type="button"
                    onClick={() => setSelectedPath(entry.path)}
                    aria-current={ativa ? "true" : undefined}
                    className="flex w-full min-w-0 items-center gap-2.5 px-2 py-2 text-left focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
                  >
                    <span className="lateral-avatar grid h-8 w-8 shrink-0 place-items-center border border-border-subtle">
                      <AgentIcon provider={entry.provider} size={16} />
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="flex min-w-0 items-baseline gap-2">
                        <span className={`min-w-0 flex-1 truncate text-xs leading-4 ${ativa ? "font-semibold text-text-primary" : "text-text-secondary"}`}>
                          {entry.title ?? entry.first_prompt}
                        </span>
                        <time className="shrink-0 text-[10px] leading-4 text-text-muted tabular-nums" title={formatDate(entry.updated_at_ms)}>
                          {quando(entry.updated_at_ms)}
                        </time>
                      </span>
                      <span className="mt-0.5 flex min-w-0 items-center gap-1 text-[10px] leading-4 text-text-muted">
                        <FolderIcon className="h-3 w-3 shrink-0" aria-hidden="true" />
                        <span className="truncate">{entry.cwd ? baseName(entry.cwd) : "sem pasta"}</span>
                        <span aria-hidden>·</span>
                        <span className="truncate">{entry.profile_name}</span>
                      </span>
                    </span>
                  </button>
                </li>
              );
            })}
            {entries !== null && filtered.length === 0 && (
              <li className="px-3 py-8 text-center text-xs text-text-muted">
                {entries.length === 0 ? "Nenhuma conversa gravada ainda." : "Nada encontrado com esses filtros."}
              </li>
            )}
          </ul>
          {filtered.length > limit && (
            <Button variant="ghost" size="sm" onClick={() => setLimit((value) => value + PAGE)}>
              Mostrar mais
            </Button>
          )}
        </section>

        <section aria-label="Conversa selecionada" className="historico-detalhe glass">
          {selected ? (
            <ConversationDetail
              key={selected.path}
              entry={selected}
              conversa={conversasPorSessao[selected.session_id] ?? null}
              contas={contas.filter((conta) => conta.provider === selected.provider)}
              onResume={onResume}
            />
          ) : (
            <div className="m-auto flex flex-col items-center gap-3 px-6 text-center">
              <span className="historico-vazio-icone" aria-hidden><ChatIcon className="h-6 w-6" /></span>
              <p className="text-xs text-text-secondary">Escolha uma conversa para ver as mensagens.</p>
              <p className="text-[11px] text-text-muted">Depois é só escolher a conta e retomar no terminal.</p>
            </div>
          )}
        </section>
      </div>
    </div>
  );
}

function ConversationDetail({
  entry,
  conversa,
  contas,
  onResume,
}: {
  entry: HistoryEntry;
  conversa: Conversation | null;
  /** Contas do mesmo agente da conversa. */
  contas: Profile[];
  onResume: HistoryViewProps["onResume"];
}) {
  const [conta, setConta] = useState(entry.profile_id);
  const [nome, setNome] = useState<string | null>(null);
  const [transcript, setTranscript] = useState<HistoryTranscript | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [resuming, setResuming] = useState(false);
  const [saidas, setSaidas] = useState<AcaoDeRetomada[]>([]);

  useEffect(() => {
    let cancelled = false;
    readTranscript(entry)
      .then((result) => {
        if (!cancelled) setTranscript(result);
      })
      .catch((reason) => {
        if (!cancelled) setError(errorText(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [entry]);

  async function resume(opcoes?: OpcoesDeRetomada) {
    setResuming(true);
    setError(null);
    setSaidas([]);
    try {
      await onResume(entry, conta, opcoes);
    } catch (reason) {
      setError(errorText(reason));
      setSaidas(reason instanceof RetomadaError ? reason.acoes : []);
    } finally {
      setResuming(false);
    }
  }

  const outraConta = conta !== entry.profile_id;
  const opcoesDeConta = (contas.length > 0 ? contas : [{ id: entry.profile_id, name: entry.profile_name, authenticated: true } as Profile])
    .map((item) => ({
      value: item.id,
      label: item.name,
      icon: <AgentIcon provider={entry.provider} size={12} className="shrink-0" />,
      hint: item.id === entry.profile_id ? "conta original" : item.authenticated ? "conectada" : "sem login",
    }));

  return (
    <>
      <header className="historico-detalhe-topo">
        <div className="min-w-0 flex-1 space-y-1">
          {conversa ? (
            <EditableLabel
              value={nome ?? conversa.title}
              className="block text-sm font-semibold text-text-primary"
              label={`Renomear conversa ${nome ?? conversa.title}`}
              onCommit={(valor) => {
                setNome(valor);
                void renameConversation(conversa.id, valor).catch(() => setNome(null));
              }}
            />
          ) : (
            <h2 className="text-sm font-semibold text-text-primary">{entry.title ?? entry.first_prompt}</h2>
          )}
          <p className="historico-detalhe-meta">
            <span title={entry.cwd ?? undefined}><FolderIcon className="h-3 w-3" aria-hidden="true" />{entry.cwd ? baseName(entry.cwd) : "Pasta não registrada"}</span>
            <span><ClockIcon className="h-3 w-3" aria-hidden="true" />{formatDate(entry.updated_at_ms)}</span>
            {transcript && <span><ChatIcon className="h-3 w-3" aria-hidden="true" />{transcript.total} mensagens</span>}
          </p>
        </div>
        <div className="historico-retomar">
          <label htmlFor="historico-conta" className="historico-retomar-rotulo">Abrir na conta</label>
          <div className="flex items-stretch gap-2">
            <Select id="historico-conta" size="sm" value={conta} options={opcoesDeConta} onChange={setConta} className="min-w-[11rem]" />
            <Button size="sm" variant="primary" onClick={() => void resume()} loading={resuming}>
              <PlayIcon className="h-3.5 w-3.5" aria-hidden="true" />
              Retomar no terminal
            </Button>
          </div>
          {outraConta && (
            <p className="text-[10px] text-text-muted">A conversa é copiada para essa conta antes de abrir.</p>
          )}
        </div>
      </header>
      {error && (
        <div role="alert" className="mx-4 mt-3 border-l-2 border-danger pl-3 text-xs text-danger">
          <p>{error}</p>
          {saidas.length > 0 && (
            <div className="mt-2 flex flex-wrap gap-2">
              {saidas.map((saida) => (
                <Button key={saida.rotulo} size="sm" variant="glass" title={saida.dica} disabled={resuming}
                  onClick={() => void resume(saida.opcoes)}>
                  {saida.rotulo}
                </Button>
              ))}
            </div>
          )}
        </div>
      )}
      <div className="historico-mensagens">
        {transcript === null && !error && <p className="text-xs text-text-muted">Carregando mensagens…</p>}
        {transcript && transcript.total > transcript.messages.length && (
          <p className="text-center text-[11px] text-text-muted">
            Mostrando as últimas {transcript.messages.length} de {transcript.total} mensagens.
          </p>
        )}
        {transcript?.messages.map((message, index) => (
          <article key={index} className={`historico-msg historico-msg-${message.role === "user" ? "voce" : "agente"}`}>
            <p className="historico-msg-autor">
              {message.role !== "user" && <AgentIcon provider={entry.provider} size={12} className="shrink-0" />}
              {message.role === "user" ? "Você" : entry.provider === "claude" ? "Claude" : "Codex"}
            </p>
            <div className="prose prose-invert max-w-none text-xs">
              <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeSanitize]}>
                {message.text}
              </ReactMarkdown>
            </div>
          </article>
        ))}
      </div>
    </>
  );
}
