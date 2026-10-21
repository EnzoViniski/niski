// Fork (Editor de vídeo): a tela de edição — biblioteca, preview, propriedades, chat e timeline.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Button } from "../../components/ui";
import { ArrowLeftIcon, ClockIcon, PlayIcon, StopIcon } from "../../components/ui/PixelIcon";
import * as disco from "./armazenamento";
import { Biblioteca } from "./Biblioteca";
import { ChatDoVideo } from "./ChatDoVideo";
import {
  alternarOculto, apagar, aparar, definirDuracaoTotal, dividir, duplicar, duracaoVisivel, inserirMidia, inserirTexto, lerComposicao,
  moverClipe, mudarClipe, type Composicao,
} from "./composicao";
import { carregarEndereco, urlDoProjeto } from "./enderecos";
import { Exportar } from "./Exportar";
import { duracaoDaMidia } from "./midiaNoNavegador";
import { Preview, type ControleDoPreview } from "./Preview";
import { Propriedades } from "./Propriedades";
import { criarRelogio, formatarTempo, useRelogio, type RelogioDoPreview } from "./relogio";
import { Timeline, type Colega } from "./Timeline";
import { useProjeto } from "./useProjeto";
import { SessaoADoisDialogo } from "./aDois/SessaoADois";
import { useSessaoADois } from "./aDois/useSessaoADois";
import type { Midia, VersaoDoHistorico } from "./tipos";

/** Primeira faixa sem nada entre `inicio` e `inicio + dur` (áudio e imagem não se misturam por acaso). */
export function faixaLivre(comp: Composicao, inicio: number, dur: number, duracoes: Record<string, number>, tipo: string) {
  const ocupadas = new Map<number, string>();
  for (const c of comp.clipes) {
    const fim = c.inicio + duracaoVisivel(c, duracoes);
    if (c.inicio < inicio + dur - 1e-3 && fim > inicio + 1e-3) ocupadas.set(c.faixa, c.tipo);
  }
  const doMesmoTipo = (f: number) => comp.clipes.filter((c) => c.faixa === f).every((c) => (c.tipo === "audio") === (tipo === "audio"));
  for (let f = 0; f < 64; f++) if (!ocupadas.has(f) && doMesmoTipo(f)) return f;
  return Math.max(0, ...comp.clipes.map((c) => c.faixa)) + 1;
}

function emCampoDeTexto(e: KeyboardEvent) {
  const alvo = e.target as HTMLElement | null;
  return !!alvo && (alvo.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(alvo.tagName));
}

export interface ColaboracaoNoEditor {
  colegas: Colega[];
  /** Nome de quem está com o agente agora, se não for eu. */
  agenteCom: string | null;
  meuNome?: string;
  aoMudarSelecao?: (chave: string | null) => void;
  aoMudarTempo?: (t: number) => void;
  /** Pede a vez do agente (sessão a dois). `false` = outra pessoa está com ele. */
  pegarAgente?: () => boolean;
  soltarAgente?: () => void;
}

export function Editor({ id, onVoltar }: { id: string; onVoltar: () => void }) {
  const acoes = useProjeto(id);
  const { estado } = acoes;
  const sessao = useSessaoADois(id, acoes);
  const colaboracao: ColaboracaoNoEditor | undefined = sessao.colaboracao;
  const [dialogoADois, setDialogoADois] = useState(false);
  const relogio = useMemo<RelogioDoPreview>(() => criarRelogio(), []);
  const preview = useRef<ControleDoPreview>(null);
  const [selecionado, setSelecionado] = useState<string | null>(null);
  const [duracoes, setDuracoes] = useState<Record<string, number>>({});
  const [agenteRodando, setAgenteRodando] = useState(false);
  const [aviso, setAviso] = useState<string | null>(null);
  const [erroDoPreview, setErroDoPreview] = useState<string | null>(null);
  const [exportando, setExportando] = useState(false);
  const [versoes, setVersoes] = useState<VersaoDoHistorico[] | null>(null);
  const [textoNovo, setTextoNovo] = useState<string | null>(null);
  const [tokenPronto, setTokenPronto] = useState(false);

  useEffect(() => { void carregarEndereco().then(() => setTokenPronto(true)).catch(() => undefined); }, []);

  const comp = useMemo<Composicao>(() => {
    try { return lerComposicao(estado.html ?? ""); } catch { return { raiz: null, clipes: [] }; }
  }, [estado.html]);
  const clipe = comp.clipes.find((c) => c.chave === selecionado) ?? null;
  const bloqueado = agenteRodando || !!estado.ocupado;

  // Duração das mídias sem `data-duration` (lida só dos metadados).
  useEffect(() => {
    if (!tokenPronto) return;
    let vivo = true;
    const fontes = [...new Set(comp.clipes.filter((c) => (c.tipo === "video" || c.tipo === "audio") && c.src && !/^https?:/i.test(c.src)).map((c) => `${c.tipo}|${c.src}`))];
    for (const f of fontes) {
      const [tipo, src] = f.split("|") as ["video" | "audio", string];
      if (duracoes[src] !== undefined) continue;
      const url = urlDoProjeto(id, src.replace(/^\.\//, ""));
      if (!url) continue;
      void duracaoDaMidia(url, tipo).then((d) => { if (vivo && d !== undefined) setDuracoes((x) => ({ ...x, [src]: d })); });
    }
    return () => { vivo = false; };
  }, [comp.clipes, id, tokenPronto, duracoes]);

  useEffect(() => { colaboracao?.aoMudarSelecao?.(selecionado); }, [selecionado, colaboracao]);
  useEffect(() => {
    if (!colaboracao?.aoMudarTempo) return;
    let ultimo = -1;
    return relogio.assinar(() => {
      const t = relogio.ler().tempo;
      if (Math.abs(t - ultimo) > 0.2) { ultimo = t; colaboracao.aoMudarTempo?.(t); }
    });
  }, [relogio, colaboracao]);

  const editar = useCallback((op: (html: string) => string, motivo: string) => {
    if (bloqueado || estado.html === null) return;
    try {
      void acoes.editarHtml(op(estado.html), motivo);
      setAviso(null);
    } catch (e) { setAviso(e instanceof Error ? e.message : String(e)); }
  }, [acoes, bloqueado, estado.html]);

  const porNaTimeline = useCallback(async (m: Midia, inicio?: number, faixa?: number) => {
    const t = inicio ?? relogio.ler().tempo;
    const url = urlDoProjeto(id, m.arquivo);
    const dur = (m.tipo === "video" || m.tipo === "audio") && url ? await duracaoDaMidia(url, m.tipo) : undefined;
    const f = faixa ?? faixaLivre(comp, t, dur ?? 3, duracoes, m.tipo);
    editar((html) => inserirMidia(html, { arquivo: m.arquivo, tipo: m.tipo, nome: m.nome, duracao: dur }, t, f), `“${m.nome}” na timeline em ${formatarTempo(t)}`);
  }, [comp, duracoes, editar, id, relogio]);

  const dividirNaAgulha = useCallback(() => {
    if (!clipe) return;
    const t = relogio.ler().tempo;
    editar((html) => dividir(html, clipe.chave, t, duracoes), `Dividi “${clipe.rotulo}” em ${formatarTempo(t)}`);
  }, [clipe, duracoes, editar, relogio]);

  const apagarSelecionado = useCallback(() => {
    if (!clipe) return;
    editar((html) => apagar(html, clipe.chave), `Apaguei “${clipe.rotulo}”`);
    setSelecionado(null);
  }, [clipe, editar]);

  // Atalhos de teclado (fora dos campos de texto).
  useEffect(() => {
    const aoTeclar = (e: KeyboardEvent) => {
      if (emCampoDeTexto(e) || exportando) return;
      const cmd = e.metaKey || e.ctrlKey;
      if (cmd && e.key.toLowerCase() === "z") { e.preventDefault(); if (!bloqueado) void (e.shiftKey ? acoes.refazer() : acoes.desfazer()); return; }
      if (cmd && e.key.toLowerCase() === "d" && clipe) { e.preventDefault(); editar((h) => duplicar(h, clipe.chave, duracoes), `Dupliquei “${clipe.rotulo}”`); return; }
      if (cmd) return;
      if (e.key === " ") { e.preventDefault(); preview.current?.alternar(); }
      else if (e.key === "ArrowLeft") { e.preventDefault(); preview.current?.quadros(e.shiftKey ? -10 : -1); }
      else if (e.key === "ArrowRight") { e.preventDefault(); preview.current?.quadros(e.shiftKey ? 10 : 1); }
      else if (e.key === "Home") { e.preventDefault(); preview.current?.irPara(0); }
      else if (e.key === "End") { e.preventDefault(); preview.current?.irPara(relogio.ler().duracao); }
      else if (e.key.toLowerCase() === "s" && clipe) { e.preventDefault(); dividirNaAgulha(); }
      else if ((e.key === "Backspace" || e.key === "Delete") && clipe) { e.preventDefault(); apagarSelecionado(); }
      else if (e.key === "Escape") setSelecionado(null);
    };
    window.addEventListener("keydown", aoTeclar);
    return () => window.removeEventListener("keydown", aoTeclar);
  }, [acoes, apagarSelecionado, bloqueado, clipe, dividirNaAgulha, duracoes, editar, exportando, relogio]);

  const contexto = useCallback(() => {
    const t = formatarTempo(relogio.ler().tempo);
    const sel = clipe ? `; clipe selecionado na timeline: id "${clipe.id ?? "(sem id)"}" (${clipe.tipo}, ${formatarTempo(clipe.inicio)} a ${formatarTempo(clipe.inicio + duracaoVisivel(clipe, duracoes))}, ${clipe.rotulo})` : "";
    return `[Contexto do editor: agulha em ${t}${sel}. Arquivo principal: index.html]`;
  }, [clipe, duracoes, relogio]);

  const aoRodarAgente = useCallback((r: boolean) => {
    setAgenteRodando(r);
    if (r) colaboracao?.pegarAgente?.(); else colaboracao?.soltarAgente?.();
  }, [colaboracao]);

  if (estado.erro && !estado.projeto) {
    return (
      <div className="p-6 space-y-3">
        <p role="alert" className="video-erro">Não consegui abrir este vídeo: {estado.erro}</p>
        <Button size="sm" onClick={onVoltar}>← Vídeos</Button>
      </div>
    );
  }
  if (!estado.projeto || estado.html === null) return <div className="p-6 font-mono text-xs text-text-muted">Abrindo o vídeo…</div>;
  const p = estado.projeto;
  const raiz = comp.raiz;

  return (
    <div className="video-editor">
      <header className="video-editor-topo">
        <Button size="sm" variant="ghost" onClick={onVoltar} aria-label="Voltar para os vídeos"><ArrowLeftIcon size={14} /> Vídeos</Button>
        <NomeDoVideo nome={p.nome} onMudar={(nome) => void acoes.mudarProjeto({ nome })} />
        <span className="video-etiqueta">{p.formato} · {raiz ? `${raiz.largura}×${raiz.altura}` : "sem root"}</span>
        {estado.ocupado && <span className="video-etiqueta is-ocupado">{estado.ocupado}</span>}
        <span className="flex-1" />
        <Button size="sm" variant={sessao.papel ? "primary" : "ghost"} onClick={() => setDialogoADois(true)} aria-pressed={!!sessao.papel}
          title={sessao.papel ? (sessao.conexao === "caiu" ? "Conexão caiu; tentando de novo" : "Sessão a dois aberta") : "Editar junto com outra pessoa pelo Tailscale"}>
          {sessao.papel ? (sessao.conexao === "caiu" ? "A dois · reconectando" : `A dois${sessao.colegas.length ? ` · ${sessao.colegas.map((c) => c.nome).join(", ")}` : ""}`) : "A dois"}
          {sessao.pedidos.length > 0 && <span className="video-ponto" aria-label={`${sessao.pedidos.length} pedido(s) para entrar`} />}
        </Button>
        <Button size="sm" variant="ghost" disabled={!estado.podeDesfazer || bloqueado} onClick={() => void acoes.desfazer()} title="Desfazer (⌘Z)">Desfazer</Button>
        <Button size="sm" variant="ghost" disabled={!estado.podeRefazer || bloqueado} onClick={() => void acoes.refazer()} title="Refazer (⇧⌘Z)">Refazer</Button>
        <div className="video-historico">
          <Button size="sm" variant="ghost" aria-expanded={versoes !== null} onClick={() => setVersoes(versoes ? null : [])} title="Versões guardadas"><ClockIcon size={14} /> Versões</Button>
          {versoes !== null && <ListaDeVersoes projeto={id} onVoltarPara={(n) => { setVersoes(null); void acoes.voltarPara(n); }} onFechar={() => setVersoes(null)} />}
        </div>
        <Button size="sm" variant="primary" onClick={() => setExportando(true)}>Exportar</Button>
      </header>

      <aside className="video-coluna is-esquerda">
        <Biblioteca projeto={id} acoes={acoes} bloqueado={bloqueado} onPorNaTimeline={(m) => void porNaTimeline(m)} />
      </aside>

      <main className="video-centro">
        {raiz ? (
          <Preview ref={preview} projeto={id} versao={estado.versaoDoPreview} largura={raiz.largura} altura={raiz.altura} fps={raiz.fps} relogio={relogio} onErro={setErroDoPreview} />
        ) : <p className="video-erro">A composição não tem root (`data-composition-id`). Peça ao agente para arrumar o index.html.</p>}
        <Transporte relogio={relogio} controle={preview} fps={raiz?.fps ?? 30}
          onTexto={() => setTextoNovo("")} desligado={bloqueado} />
        {textoNovo !== null && (
          <form className="video-texto-novo" onSubmit={(e) => {
            e.preventDefault();
            const t = textoNovo.trim();
            if (t) { const ini = relogio.ler().tempo; editar((h) => inserirTexto(h, t, ini, 2.5, faixaLivre(comp, ini, 2.5, duracoes, "texto")), `Texto “${t.slice(0, 30)}”`); }
            setTextoNovo(null);
          }}>
            <input className="field px-2.5 py-1.5 text-xs" autoFocus placeholder="Texto na tela (entra na agulha, 2,5 s)" value={textoNovo}
              onChange={(e) => setTextoNovo(e.target.value)} onKeyDown={(e) => { e.stopPropagation(); if (e.key === "Escape") setTextoNovo(null); }} />
            <Button size="sm" variant="primary" type="submit">Pôr</Button>
          </form>
        )}
        {(aviso || erroDoPreview || estado.erro) && (
          <p className="video-aviso" role="alert">{aviso ?? estado.erro ?? erroDoPreview}
            <button type="button" className="video-mini-botao" aria-label="Fechar aviso" onClick={() => { setAviso(null); acoes.limparErro(); setErroDoPreview(null); }}>×</button>
          </p>
        )}
        <Propriedades comp={comp} clipe={clipe} duracoes={duracoes} bloqueado={bloqueado}
          onMudar={(chave, mudancas, motivo, novoFim) => editar((h) => mudarClipe(h, chave, mudancas, novoFim), motivo)}
          onDuracaoTotal={(s) => editar((h) => definirDuracaoTotal(h, s), `Duração total: ${s} s`)}
          onDividir={dividirNaAgulha}
          onDuplicar={() => clipe && editar((h) => duplicar(h, clipe.chave, duracoes), `Dupliquei “${clipe.rotulo}”`)}
          onApagar={apagarSelecionado}
          onOcultar={() => clipe && editar((h) => alternarOculto(h, clipe.chave), clipe.oculto ? `Mostrei “${clipe.rotulo}”` : `Escondi “${clipe.rotulo}”`)} />
      </main>

      <aside className="video-coluna is-direita">
        <ChatDoVideo projeto={id} acoes={acoes} contexto={contexto} onRodando={aoRodarAgente}
          autor={colaboracao?.meuNome} bloqueadoPor={colaboracao?.agenteCom ?? null} />
      </aside>

      <section className="video-rodape">
        <Timeline comp={comp} duracoes={duracoes} relogio={relogio} selecionado={selecionado} onSelecionar={setSelecionado}
          onIrPara={(t) => preview.current?.irPara(t)} somenteLeitura={bloqueado} colegas={colaboracao?.colegas}
          onMover={(chave, inicio, faixa) => editar((h) => moverClipe(h, chave, inicio, faixa, duracoes), `Movi “${comp.clipes.find((c) => c.chave === chave)?.rotulo ?? chave}” para ${formatarTempo(inicio)}`)}
          onAparar={(chave, lado, t) => editar((h) => aparar(h, chave, lado, t, duracoes), `Cortei ${lado === "inicio" ? "o começo" : "o fim"} de “${comp.clipes.find((c) => c.chave === chave)?.rotulo ?? chave}”`)}
          onSoltarMidia={(midiaId, inicio, faixa) => { const m = estado.midias.itens.find((x) => x.id === midiaId); if (m) void porNaTimeline(m, inicio, faixa); }} />
      </section>

      {exportando && <Exportar projeto={id} nome={p.nome} onFechar={() => setExportando(false)} />}
      {dialogoADois && <SessaoADoisDialogo sessao={sessao} onFechar={() => setDialogoADois(false)} />}
    </div>
  );
}

/** Grava o nome só ao sair do campo (ou Enter), não a cada tecla. */
function NomeDoVideo({ nome, onMudar }: { nome: string; onMudar: (n: string) => void }) {
  const [texto, setTexto] = useState(nome);
  useEffect(() => setTexto(nome), [nome]);
  const gravar = () => { const t = texto.trim(); if (t && t !== nome) onMudar(t); else setTexto(nome); };
  return (
    <input className="video-nome" value={texto} aria-label="Nome do vídeo" onChange={(e) => setTexto(e.target.value)} onBlur={gravar}
      onKeyDown={(e) => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} />
  );
}

function Transporte({ relogio, controle, fps, onTexto, desligado }: { relogio: RelogioDoPreview; controle: React.RefObject<ControleDoPreview | null>; fps: number; onTexto: () => void; desligado: boolean }) {
  const { tempo, duracao, tocando, pronto } = useRelogio(relogio);
  return (
    <div className="video-transporte">
      <button type="button" className="video-mini-botao" onClick={() => controle.current?.irPara(0)} aria-label="Voltar ao começo" disabled={!pronto}>⏮</button>
      <button type="button" className="video-mini-botao" onClick={() => controle.current?.quadros(-1)} aria-label="Quadro anterior" disabled={!pronto}>‹</button>
      <button type="button" className="video-tocar" onClick={() => controle.current?.alternar()} aria-label={tocando ? "Pausar" : "Tocar"} disabled={!pronto}>
        {tocando ? <StopIcon size={14} /> : <PlayIcon size={14} />}
      </button>
      <button type="button" className="video-mini-botao" onClick={() => controle.current?.quadros(1)} aria-label="Próximo quadro" disabled={!pronto}>›</button>
      <span className="video-tempo" aria-live="off">{formatarTempo(tempo)} <small>/ {formatarTempo(duracao)} · quadro {Math.round(tempo * fps)}</small></span>
      <span className="flex-1" />
      <Button size="sm" variant="ghost" disabled={desligado} onClick={onTexto}>＋ Texto</Button>
    </div>
  );
}

function ListaDeVersoes({ projeto, onVoltarPara, onFechar }: { projeto: string; onVoltarPara: (n: number) => void; onFechar: () => void }) {
  const [lista, setLista] = useState<VersaoDoHistorico[] | null>(null);
  useEffect(() => { void disco.listarVersoes(projeto).then(setLista).catch(() => setLista([])); }, [projeto]);
  return (
    <div className="video-versoes glass" role="dialog" aria-label="Versões guardadas" onKeyDown={(e) => { if (e.key === "Escape") onFechar(); e.stopPropagation(); }}>
      <p className="video-dica">Antes de cada mudança o app guarda uma versão. Voltar para uma delas guarda a atual também.</p>
      {lista === null && <p className="video-dica">Carregando…</p>}
      {lista?.length === 0 && <p className="video-vazio">Nenhuma versão ainda.</p>}
      <ul>
        {lista?.map((v) => (
          <li key={v.n}>
            <span><b>{!v.motivo ? `Versão ${v.n}` : /^antes/i.test(v.motivo) ? v.motivo : `Antes de: ${v.motivo}`}</b><small>{new Date(v.quando).toLocaleString("pt-BR", { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" })}</small></span>
            <Button size="sm" variant="ghost" onClick={() => onVoltarPara(v.n)}>Voltar</Button>
          </li>
        ))}
      </ul>
    </div>
  );
}
