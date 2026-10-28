// Fork (Editor de vídeo): os vídeos (projetos) e as pastas de referências.
import { useEffect, useMemo, useState } from "react";
import { Button } from "../../components/ui";
import { ClapperboardIcon, FolderIcon, FolderPlusIcon, TrashIcon } from "../../components/ui/PixelIcon";
import * as disco from "./armazenamento";
import { CORES_DAS_PASTAS } from "./PastaDeReferencias";
import { escolherPastaDoComputador, nomeDaPasta as nomeCurto } from "./PastaDoComputador";
import { EntrarNaSessao } from "./aDois/EntrarNaSessao";
import type { ItemDaLista, PastaDeReferencia } from "./tipos";

function quando(ms: number) {
  if (!ms) return "";
  return new Date(ms).toLocaleString("pt-BR", { day: "2-digit", month: "short", hour: "2-digit", minute: "2-digit" });
}

// Pastas do computador que a pessoa já usou (só conveniência; cada vídeo guarda a dele).
const PASTAS_LEMBRADAS = "medza.video.pastas";
const PASTA_ATIVA = "medza.video.pasta-ativa";
function lerLista(): string[] { try { const v = JSON.parse(localStorage.getItem(PASTAS_LEMBRADAS) ?? "[]"); return Array.isArray(v) ? v.filter((x) => typeof x === "string") : []; } catch { return []; } }
function gravar(chave: string, valor: string | null) { try { if (valor === null) localStorage.removeItem(chave); else localStorage.setItem(chave, valor); } catch { /* sem armazenamento */ } }
function lerAtiva() { try { return localStorage.getItem(PASTA_ATIVA); } catch { return null; } }

export function ListaDeVideos({ onAbrir, onNovo, onAbrirPasta, onFechar, extra }: {
  onAbrir: (id: string) => void; onNovo: (pastaDoComputador?: string) => void; onAbrirPasta: (id: string) => void; onFechar: () => void; extra?: React.ReactNode;
}) {
  const [lista, setLista] = useState<ItemDaLista[] | null>(null);
  const [pastas, setPastas] = useState<PastaDeReferencia[]>([]);
  const [erro, setErro] = useState<string | null>(null);
  const [confirmar, setConfirmar] = useState<string | null>(null);
  const [nomeDaPasta, setNomeDaPasta] = useState("");
  const [entrando, setEntrando] = useState(false);
  const [lembradas, setLembradas] = useState<string[]>(lerLista);
  /** Filtro: `null` = todos os vídeos; caminho = só os desta pasta do computador. */
  const [ativa, setAtiva] = useState<string | null>(lerAtiva);
  const pastasDoPc = useMemo(() => [...new Set([...lembradas, ...(lista ?? []).map((i) => i.projeto.pastaDoComputador).filter((c): c is string => !!c)])], [lembradas, lista]);
  const visiveis = useMemo(() => (ativa ? lista?.filter((i) => i.projeto.pastaDoComputador === ativa) : lista) ?? null, [ativa, lista]);
  const escolherAtiva = (c: string | null) => { setAtiva(c); gravar(PASTA_ATIVA, c); };
  const adicionarPasta = async () => {
    try {
      const c = await escolherPastaDoComputador();
      if (!c) return;
      const nova = [c, ...lembradas.filter((x) => x !== c)].slice(0, 12);
      setLembradas(nova); gravar(PASTAS_LEMBRADAS, JSON.stringify(nova));
      escolherAtiva(c);
    } catch (e) { setErro(String(e)); }
  };
  const esquecer = (c: string) => {
    const nova = lembradas.filter((x) => x !== c);
    setLembradas(nova); gravar(PASTAS_LEMBRADAS, JSON.stringify(nova));
    if (ativa === c) escolherAtiva(null);
  };

  const carregar = () => {
    void disco.listarProjetos().then(setLista).catch((e) => setErro(String(e)));
    void disco.listarPastas().then(setPastas).catch(() => undefined);
  };
  useEffect(carregar, []);

  const criarPasta = async () => {
    const nome = nomeDaPasta.trim();
    if (!nome) return;
    try {
      const p = await disco.criarPasta({ nome, cor: CORES_DAS_PASTAS[pastas.length % CORES_DAS_PASTAS.length] });
      setNomeDaPasta("");
      onAbrirPasta(p.id);
    } catch (e) { setErro(String(e)); }
  };

  return (
    <div className="video-lista">
      <header className="video-novo-topo">
        <h1><ClapperboardIcon size={18} aria-hidden="true" /> Edição de vídeo</h1>
        <span className="flex-1" />
        {extra}
        <Button size="sm" variant="ghost" onClick={onFechar}>Fechar</Button>
        <Button size="sm" variant="glass" onClick={() => setEntrando(true)}>Entrar numa sessão a dois</Button>
        <Button size="sm" variant="primary" onClick={() => onNovo(ativa ?? undefined)} title={ativa ? `Novo vídeo na pasta ${nomeCurto(ativa)}` : undefined}>
          {ativa ? `Novo vídeo em “${nomeCurto(ativa)}”` : "Novo vídeo"}
        </Button>
      </header>
      {erro && <p className="video-erro mx-6" role="alert">{erro}</p>}
      {entrando && <EntrarNaSessao onFechar={() => setEntrando(false)} onEntrou={(id) => { setEntrando(false); onAbrir(id); }} />}
      <section className="video-lista-secao">
        <h2 className="video-subtitulo">Pasta do computador</h2>
        <p className="video-dica">Escolha a pasta da sua marca ou do cliente (identidade visual, logos, informações). Os vídeos dela usam esse material; cada vídeo escolhe as próprias pastas de referências.</p>
        <div className="video-pastas-escolha" role="group" aria-label="Pasta do computador">
          <button type="button" aria-pressed={ativa === null} className={`video-pasta-chip${ativa === null ? " is-ativa" : ""}`} onClick={() => escolherAtiva(null)}>Todos os vídeos</button>
          {pastasDoPc.map((c) => (
            <span key={c} className="video-pasta-chip-grupo">
              <button type="button" aria-pressed={ativa === c} title={c} className={`video-pasta-chip${ativa === c ? " is-ativa" : ""}`} onClick={() => escolherAtiva(c)}>
                <FolderIcon size={12} aria-hidden="true" />{nomeCurto(c)}<small>{lista?.filter((i) => i.projeto.pastaDoComputador === c).length ?? 0}</small>
              </button>
              {lembradas.includes(c) && !lista?.some((i) => i.projeto.pastaDoComputador === c) && (
                <button type="button" className="video-mini-botao" aria-label={`Esquecer ${nomeCurto(c)}`} title="Tirar da lista (a pasta não é apagada)" onClick={() => esquecer(c)}>×</button>
              )}
            </span>
          ))}
          <button type="button" className="video-pasta-chip" onClick={() => void adicionarPasta()}><FolderPlusIcon size={12} aria-hidden="true" />Escolher pasta…</button>
        </div>
      </section>
      <section className="video-lista-secao">
        <h2 className="video-subtitulo">{ativa ? `Vídeos de “${nomeCurto(ativa)}”` : "Vídeos"}</h2>
        {lista === null && !erro && <p className="video-dica">Carregando…</p>}
        {visiveis?.length === 0 && (
          <div className="video-chat-vazio">
            <p>{ativa ? "Nenhum vídeo nesta pasta ainda." : "Nenhum vídeo ainda. Crie um, arraste as referências e peça ao agente a primeira versão."}</p>
            <Button size="sm" variant="primary" onClick={() => onNovo(ativa ?? undefined)}>Criar o primeiro vídeo</Button>
          </div>
        )}
        <div className="video-grade">
          {visiveis?.map(({ id, projeto }) => (
            <div key={id} className="video-cartao glass">
              <button type="button" className="video-cartao-abrir" onClick={() => onAbrir(id)}>
                <span className="video-cartao-formato" style={{ aspectRatio: `${projeto.largura || 16} / ${projeto.altura || 9}` }} aria-hidden="true" />
                <span className="video-cartao-texto"><b>{projeto.nome}</b><small>{projeto.aDois ? `a dois com ${projeto.aDois.host}` : projeto.formato}{!ativa && projeto.pastaDoComputador ? ` · ${nomeCurto(projeto.pastaDoComputador)}` : ""} · {quando(projeto.atualizadoEm)}</small></span>
              </button>
              {confirmar === id ? (
                <span className="video-cartao-acoes">
                  <Button size="sm" variant="danger" onClick={() => void disco.apagarProjeto(id).then(() => { setConfirmar(null); carregar(); }).catch((e) => setErro(String(e)))}>Apagar</Button>
                  <Button size="sm" variant="ghost" onClick={() => setConfirmar(null)}>Não</Button>
                </span>
              ) : (
                <button type="button" className="video-mini-botao video-cartao-lixo" aria-label={`Apagar ${projeto.nome}`} title="Vai para a lixeira do app (a pasta não some)" onClick={() => setConfirmar(id)}><TrashIcon size={12} /></button>
              )}
            </div>
          ))}
        </div>
      </section>
      <section className="video-lista-secao">
        <h2 className="video-subtitulo">Pastas de referências</h2>
        <p className="video-dica">Guarde vídeos, fotos, fontes e links de um tipo de vídeo (ex.: “Reels da Medza”). Na hora de criar, é só escolher a pasta.</p>
        <div className="video-grade">
          {pastas.map((p) => (
            <button key={p.id} type="button" className="video-cartao glass video-cartao-abrir" onClick={() => onAbrirPasta(p.id)}>
              <span className="video-pasta-icone" style={{ color: p.cor, borderColor: `${p.cor}66`, background: `${p.cor}1f` }}><FolderIcon size={20} aria-hidden="true" /></span>
              <span className="video-cartao-texto"><b>{p.nome}</b><small>{p.itens.length} arquivo{p.itens.length === 1 ? "" : "s"} · {p.links.length} link{p.links.length === 1 ? "" : "s"}</small></span>
            </button>
          ))}
          <div className="video-cartao glass video-pasta-nova">
            <FolderPlusIcon size={18} aria-hidden="true" />
            <input className="field px-2.5 py-1.5 text-xs" placeholder="Nome da pasta nova" value={nomeDaPasta} onChange={(e) => setNomeDaPasta(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") void criarPasta(); }} />
            <Button size="sm" variant="glass" disabled={!nomeDaPasta.trim()} onClick={() => void criarPasta()}>Criar</Button>
          </div>
        </div>
      </section>
    </div>
  );
}
