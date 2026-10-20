import { useCallback, useEffect, useId, useRef, useState } from "react";
import { actionKey } from "../../mobile/api";
import { useSession } from "../terminal/sessionProviders";
import { aplicarRevisao, enviarComentarios, listarRevisao, montarComentarios, type Acao, type Arquivo, type Comentario, type Linha, type Trecho } from "./revisaoService";
import "./revisao.css";
// Fork: revisão oficial em agente separado (3C).
import { RevisarComAgente } from "./RevisarComAgente";
import type { TokenRealce } from "./realce";

const chaveArquivo = (a: Arquivo) => `${a.staged}:${a.caminho}`;
const mensagem = (erro: unknown) => erro instanceof Error ? erro.message : String(erro);

export function RevisaoPane({ projectPath, sessionId, onAbrirSessao }: { projectPath: string; sessionId?: string; onAbrirSessao?: (id: string, titulo: string) => void }) {
  // Uma sessão pode trabalhar numa worktree diferente da pasta cadastrada no projeto.
  const sessao = useSession(sessionId);
  const pasta = sessao?.cwd ?? projectPath;
  return <Revisao key={`${sessionId}:${pasta}`} pasta={pasta} sessionId={sessionId} titulo={sessao?.name} onAbrirSessao={onAbrirSessao} />;
}

function Revisao({ pasta, sessionId, titulo, onAbrirSessao }: { pasta: string; sessionId?: string; titulo?: string; onAbrirSessao?: (id: string, titulo: string) => void }) {
  const campoComentario = useId();
  const [staged, setStaged] = useState(false);
  const [arquivos, setArquivos] = useState<Arquivo[]>([]);
  const [selecionado, setSelecionado] = useState<string | null>(null);
  const [carregando, setCarregando] = useState(true);
  const [ocupado, setOcupado] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  const [aviso, setAviso] = useState("");
  const [comentarios, setComentarios] = useState<Comentario[]>([]);
  const [editando, setEditando] = useState<Comentario | null>(null);
  const [confirmacao, setConfirmacao] = useState<{ arquivo: Arquivo; trecho: number | null } | null>(null);
  const geracao = useRef(0);
  const envio = useRef<{ texto: string; chave: string } | null>(null);

  const carregar = useCallback(async () => {
    const atual = ++geracao.current;
    setCarregando(true); setErro(null);
    try {
      const lista = await listarRevisao(pasta, staged);
      if (atual !== geracao.current) return;
      setArquivos(lista);
      setSelecionado(prev => lista.some(a => chaveArquivo(a) === prev) ? prev : lista[0] ? chaveArquivo(lista[0]) : null);
    } catch (e) { if (atual === geracao.current) setErro(mensagem(e)); }
    finally { if (atual === geracao.current) setCarregando(false); }
  }, [pasta, staged]);

  useEffect(() => { void carregar(); return () => { geracao.current++; }; }, [carregar]);
  const arquivo = arquivos.find(a => chaveArquivo(a) === selecionado);

  async function aplicar(alvo: Arquivo, acao: Acao, trecho: number | null) {
    setOcupado(true); setErro(null); setAviso(""); setConfirmacao(null);
    try {
      await aplicarRevisao(pasta, alvo, acao, trecho);
      await carregar();
    } catch (e) { setErro(mensagem(e)); }
    finally { setOcupado(false); }
  }

  function comentar(a: Arquivo, trecho: Trecho, linha: Linha, indice: number) {
    const numero = linha.nova ?? linha.antiga;
    if (numero === null) return;
    const id = `${chaveArquivo(a)}:${trecho.patch}:${indice}`;
    setEditando(comentarios.find(c => c.id === id) ?? {
      id, arquivo: a.caminho, linha: numero, lado: linha.nova === null ? "anterior" : "atual",
      trecho: [trecho.cabecalho, ...trecho.linhas.map(l => l.tipo + l.texto)].join("\n"), texto: "",
    });
  }

  async function enviar() {
    if (!sessionId || comentarios.length === 0) return;
    setOcupado(true); setErro(null); setAviso("");
    const texto = montarComentarios(comentarios);
    if (envio.current?.texto !== texto) envio.current = { texto, chave: actionKey() };
    try {
      await enviarComentarios(sessionId, texto, envio.current.chave);
      setComentarios([]); envio.current = null;
      setAviso("Comentários enviados ao agente da aba de origem.");
    } catch (e) { setErro(mensagem(e)); }
    finally { setOcupado(false); }
  }

  const bloqueado = ocupado || carregando;
  return <div className="revisao">
    <header className="revisao-topo">
      <strong>Revisão</strong>
      <select aria-label="Escopo da revisão" value={staged ? "staged" : "local"} disabled={bloqueado} onChange={e => { setConfirmacao(null); setStaged(e.target.value === "staged"); }}>
        <option value="local">Não commitado</option><option value="staged">Staged</option>
      </select>
      <button disabled={bloqueado} onClick={() => void carregar()}>Atualizar</button>
      <span className="revisao-pasta" title={pasta}>{pasta}</span>
    </header>
    <RevisarComAgente sessionId={sessionId} onAbrirSessao={onAbrirSessao} />
    {erro && <div role="alert" className="revisao-erro">{erro}</div>}
    {aviso && <div role="status" className="p-2 text-success">{aviso}</div>}
    <div className="revisao-corpo" aria-busy={carregando}>
      <nav className="revisao-arquivos" aria-label="Arquivos alterados">
        <p className="p-2 text-text-muted">{staged ? "Alterações no índice" : "Alterações locais e staged"}</p>
        {carregando && <p className="p-2 text-text-muted">Carregando…</p>}
        {!carregando && !arquivos.length && !erro && <p className="p-2 text-text-muted">Nenhuma alteração neste escopo.</p>}
        {arquivos.map(a => <button key={chaveArquivo(a)} disabled={bloqueado} aria-current={chaveArquivo(a) === selecionado ? "true" : undefined} onClick={() => { setSelecionado(chaveArquivo(a)); setConfirmacao(null); }}>
          <span className="revisao-caminho">{a.caminho}</span>
          <span className="text-success">+{a.adicionadas}</span> <span className="text-danger">−{a.removidas}</span>
          <small>{a.status} · {a.staged ? "Staged" : "Fora de staged"}</small>
        </button>)}
      </nav>
      <main className="revisao-diff" aria-label="Diff do arquivo">
        {arquivo && <>
          <div className="revisao-acoes">
            <strong>{arquivo.caminho}</strong>
            <button disabled={bloqueado} onClick={() => void aplicar(arquivo, arquivo.staged ? "unstage" : "stage", null)}>{arquivo.staged ? "Unstage do arquivo" : "Stage do arquivo"}</button>
            <button disabled={bloqueado} onClick={() => setConfirmacao({ arquivo, trecho: null })}>Descartar arquivo</button>
          </div>
          {!arquivo.granular && <p className="p-2 text-text-muted">{arquivo.trechos.length ? "Metadados e conteúdo devem ser aplicados pelo arquivo inteiro." : "Arquivo binário, vazio ou alteração de metadados. Use as ações do arquivo."}</p>}
          {arquivo.trechos.map((t, i) => <section className="revisao-trecho" key={`${chaveArquivo(arquivo)}:${i}:${t.patch}`} aria-label={`Trecho ${i + 1}`}>
            <div className="revisao-acoes revisao-cabecalho">
              <code>{t.cabecalho}</code>
              {arquivo.granular && <>
                <button disabled={bloqueado} onClick={() => void aplicar(arquivo, arquivo.staged ? "unstage" : "stage", i)}>{arquivo.staged ? "Unstage do trecho" : "Stage do trecho"}</button>
                <button disabled={bloqueado} onClick={() => setConfirmacao({ arquivo, trecho: i })}>Descartar trecho</button>
              </>}
            </div>
            <Linhas caminho={arquivo.caminho} trecho={t} bloqueado={bloqueado} comentar={(l, n) => comentar(arquivo, t, l, n)} />
          </section>)}
        </>}
      </main>
    </div>
    {confirmacao && <div role="alertdialog" aria-label="Confirmar descarte" className="revisao-confirmar">
      <p>Descartar {confirmacao.trecho === null ? "todas as alterações desta camada de" : `o trecho ${confirmacao.trecho + 1} de`} <strong>{confirmacao.arquivo.caminho}</strong>? Esta ação apaga as alterações selecionadas{confirmacao.arquivo.staged ? " do índice e do arquivo" : " do arquivo"}.</p>
      <button disabled={ocupado} onClick={() => void aplicar(confirmacao.arquivo, "descartar", confirmacao.trecho)}>Confirmar descarte</button>
      <button disabled={ocupado} onClick={() => setConfirmacao(null)}>Cancelar</button>
    </div>}
    {editando && <form className="revisao-comentar" onSubmit={e => {
      e.preventDefault(); if (!editando.texto.trim()) return;
      setComentarios(prev => [...prev.filter(c => c.id !== editando.id), editando]); setEditando(null);
    }}>
      <label htmlFor={campoComentario}>Comentário em {editando.arquivo}:{editando.linha} ({editando.lado})</label>
      <textarea id={campoComentario} autoFocus value={editando.texto} disabled={ocupado} onChange={e => setEditando({ ...editando, texto: e.target.value })} />
      <button disabled={ocupado || !editando.texto.trim()}>Salvar comentário</button>
      <button type="button" disabled={ocupado} onClick={() => setEditando(null)}>Cancelar comentário</button>
    </form>}
    <footer className="revisao-comentarios" onKeyDown={e => { if ((e.metaKey || e.ctrlKey) && e.key === "Enter" && !ocupado && !editando) { e.preventDefault(); void enviar(); } }}>
      {comentarios.map(c => <div key={c.id} className="revisao-comentario">
        <button disabled={ocupado} onClick={() => setEditando(c)}>{c.arquivo}:{c.linha} — {c.texto}</button>
        <button disabled={ocupado} aria-label={`Remover comentário em ${c.arquivo}:${c.linha}`} onClick={() => setComentarios(prev => prev.filter(item => item.id !== c.id))}>Remover</button>
      </div>)}
      <div className="revisao-acoes">
        <span title={sessionId}>{sessionId ? `Para: ${titulo ?? "agente da aba de origem"}` : "Abra a revisão pela aba de um agente para enviar comentários."}</span>
        <button disabled={ocupado || !sessionId || !comentarios.length || !!editando} onClick={() => void enviar()}>Enviar comentários ao agente ({comentarios.length})</button>
      </div>
    </footer>
  </div>;
}

function Linhas({ caminho, trecho, bloqueado, comentar }: { caminho: string; trecho: Trecho; bloqueado: boolean; comentar: (linha: Linha, indice: number) => void }) {
  const [realce, setRealce] = useState<TokenRealce[][]>([]);
  useEffect(() => {
    let ativo = true;
    void import("./realce").then(m => m.realcar(caminho, trecho.linhas.map(l => l.texto)))
      .then(linhas => { if (ativo) setRealce(linhas); }).catch(() => { /* texto simples continua legível */ });
    return () => { ativo = false; };
  }, [caminho, trecho]);
  return <div className="revisao-linhas">{trecho.linhas.map((l, i) => <button type="button" key={i} disabled={bloqueado || (l.antiga === null && l.nova === null)}
    className={`revisao-linha ${l.tipo === "+" ? "revisao-adicao" : l.tipo === "-" ? "revisao-remocao" : ""}`}
    aria-label={`Comentar linha ${l.nova ?? l.antiga ?? "sem número"}${l.nova === null ? " anterior" : " atual"}: ${l.texto}`} onClick={() => comentar(l, i)}>
    <span className="revisao-numero">{l.antiga}</span><span className="revisao-numero">{l.nova}</span><span className="revisao-sinal">{l.tipo}</span>
    <code>{realce[i]?.length ? realce[i].map((t, n) => <span className={`revisao-token-${t.tipo}`} key={n}>{t.texto}</span>) : l.texto || " "}</code>
  </button>)}</div>;
}
