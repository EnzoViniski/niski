// Fork (Caderno): o caderno aberto — as páginas à esquerda (como abas de divisória), a folha no meio
// (pautada, quadriculada, pontilhada ou lisa, com espiral e margem) e a IA à direita.
import ReactMarkdown from "react-markdown";
import rehypeSanitize from "rehype-sanitize";
import remarkGfm from "remark-gfm";
import { useEffect, useMemo, useRef, useState, type DragEvent } from "react";
import { Button, Input, SegmentedControl } from "../../components/ui";
import { ArrowLeftIcon, TrashIcon } from "../../components/ui/PixelIcon";
import { EscolhaDaIa, textoDoErro } from "../roadmaps/comuns";
import { ROTULO_DA_LETRA, ROTULO_DO_PAPEL } from "./Estante";
import { IaDoCaderno } from "./IaDoCaderno";
import {
  caderno, digitarNaPagina, gravarPagina, lerPagina, mandarPaginaParaALixeira, moverPagina, mudarCaderno, novaPagina, renomearPagina,
  textoDaPagina, useCadernos,
} from "./loja";
import { CAPAS, SOBRE_O_TIPO, type Letra, type MetaDoCaderno, type Papel } from "./tipos";

/** Espera depois da última tecla para gravar (e grava ao sair do campo, trocar de página ou fechar). */
const ESPERA_PARA_GRAVAR_MS = 700;
const ULTIMA_PAGINA = (id: string) => `medza-caderno-pagina:${id}`;

function lembrarPagina(id: string, pagina: string) { try { localStorage.setItem(ULTIMA_PAGINA(id), pagina); } catch { /* sem localStorage */ } }
function ultimaPagina(id: string) { try { return localStorage.getItem(ULTIMA_PAGINA(id)); } catch { return null; } }

export function CadernoAberto({ id, onVoltar }: { id: string; onVoltar: () => void }) {
  useCadernos();
  const meta = caderno(id);
  const [paginaId, setPaginaId] = useState<string | null>(() => ultimaPagina(id));
  const [ajustes, setAjustes] = useState(false);
  const [iaAberta, setIaAberta] = useState(true);
  const [busca, setBusca] = useState("");
  const [erro, setErro] = useState("");
  const [arrastando, setArrastando] = useState<string | null>(null);

  const pagina = meta?.paginas.find((p) => p.id === paginaId) ?? meta?.paginas[0];
  useEffect(() => { if (pagina) lembrarPagina(id, pagina.id); }, [id, pagina]);

  // A busca olha título e texto: carrega o texto das páginas quando você começa a buscar.
  useEffect(() => { if (busca && meta) meta.paginas.forEach((p) => void lerPagina(id, p.id).catch(() => undefined)); }, [busca, id, meta]);
  const visiveis = useMemo(() => {
    if (!meta) return [];
    const termo = busca.trim().toLocaleLowerCase("pt-BR");
    if (!termo) return meta.paginas;
    return meta.paginas.filter((p) => p.titulo.toLocaleLowerCase("pt-BR").includes(termo) || (textoDaPagina(id, p.id) ?? "").toLocaleLowerCase("pt-BR").includes(termo));
  }, [meta, busca, id]);

  if (!meta) return <div className="p-6 text-xs text-text-muted">Caderno não encontrado. <button className="text-accent underline" onClick={onVoltar}>Voltar</button></div>;

  const criarPagina = () => {
    const n = meta.paginas.length + 1;
    void novaPagina(id, meta.tipo === "livro" ? `Capítulo ${n}` : `Página ${n}`).then((p) => { if (p) setPaginaId(p.id); }).catch((e) => setErro(textoDoErro(e)));
  };

  return (
    <div className={`caderno-aberto${iaAberta ? " com-ia" : ""}`} style={{ "--capa": meta.capa } as React.CSSProperties}>
      <aside className="caderno-indice" aria-label="Páginas">
        <div className="caderno-indice-topo">
          <button type="button" className="caderno-voltar" onClick={onVoltar} aria-label="Voltar para a estante"><ArrowLeftIcon size={14} /> Cadernos</button>
          <div className="caderno-indice-capa">
            <span className="caderno-indice-tipo">{SOBRE_O_TIPO[meta.tipo].emoji} {SOBRE_O_TIPO[meta.tipo].nome}</span>
            <NomeEditavel valor={meta.nome} onMudar={(nome) => void mudarCaderno(id, { nome }).catch((e) => setErro(textoDoErro(e)))} />
            {meta.tipo === "livro" && meta.autor && <span className="caderno-indice-autor">{meta.autor}</span>}
          </div>
          <Input value={busca} onChange={(e) => setBusca(e.target.value)} placeholder="Buscar nas páginas" aria-label="Buscar nas páginas" />
        </div>
        <ol className="caderno-abas">
          {visiveis.map((p, i) => (
            <li key={p.id}
              draggable={!busca}
              onDragStart={(e: DragEvent) => { setArrastando(p.id); e.dataTransfer.effectAllowed = "move"; }}
              onDragOver={(e: DragEvent) => { if (arrastando) e.preventDefault(); }}
              onDrop={(e: DragEvent) => { e.preventDefault(); if (arrastando) void moverPagina(id, arrastando, p.id).catch((er) => setErro(textoDoErro(er))); setArrastando(null); }}
              onDragEnd={() => setArrastando(null)}
              className={arrastando === p.id ? "is-arrastando" : ""}>
              <button type="button" className={`caderno-aba${p.id === pagina?.id ? " is-atual" : ""}`} onClick={() => setPaginaId(p.id)} aria-current={p.id === pagina?.id ? "page" : undefined}>
                <span className="caderno-aba-num">{busca ? "•" : i + 1}</span>
                <span className="caderno-aba-titulo">{p.titulo || "Sem título"}</span>
              </button>
            </li>
          ))}
          {busca && visiveis.length === 0 && <li className="px-3 py-2 text-xs text-text-muted">Nada encontrado.</li>}
        </ol>
        <div className="caderno-indice-rodape">
          <Button size="sm" variant="primary" onClick={criarPagina}>+ Nova página</Button>
          <Button size="sm" variant="ghost" onClick={() => setAjustes((a) => !a)} aria-expanded={ajustes}>Ajustes</Button>
        </div>
        {ajustes && <AjustesDoCaderno meta={meta} onErro={setErro} />}
        {erro && <p role="alert" className="px-3 text-xs text-danger">{erro}</p>}
      </aside>

      <main className="caderno-mesa">
        {pagina
          ? <Folha key={pagina.id} meta={meta} paginaId={pagina.id} onErro={setErro}
              onApagar={() => {
                if (!window.confirm(`Mandar a página "${pagina.titulo || "Sem título"}" para a lixeira dos cadernos?`)) return;
                const resto = meta.paginas.filter((p) => p.id !== pagina.id);
                void mandarPaginaParaALixeira(id, pagina.id).then(() => setPaginaId(resto[0]?.id ?? null)).catch((e) => setErro(textoDoErro(e)));
              }} />
          : <div className="caderno-vazio"><p>Caderno sem páginas.</p><Button variant="primary" onClick={criarPagina}>Criar a primeira página</Button></div>}
        <button type="button" className="caderno-alternar-ia" onClick={() => setIaAberta((a) => !a)} aria-expanded={iaAberta}>
          {iaAberta ? "Esconder IA →" : "← Conversar com a IA"}
        </button>
      </main>

      {iaAberta && <IaDoCaderno meta={meta} paginaId={pagina?.id} />}
    </div>
  );
}

function NomeEditavel({ valor, onMudar }: { valor: string; onMudar: (v: string) => void }) {
  const [editando, setEditando] = useState(false);
  const [texto, setTexto] = useState(valor);
  if (!editando) return <button type="button" className="caderno-indice-nome" title="Clique para renomear" onClick={() => { setTexto(valor); setEditando(true); }}>{valor}</button>;
  const salvar = () => { setEditando(false); if (texto.trim() && texto.trim() !== valor) onMudar(texto.trim().slice(0, 120)); };
  return <Input autoFocus value={texto} onChange={(e) => setTexto(e.target.value)} onBlur={salvar} onKeyDown={(e) => { if (e.key === "Enter") salvar(); if (e.key === "Escape") setEditando(false); }} aria-label="Nome do caderno" />;
}

function AjustesDoCaderno({ meta, onErro }: { meta: MetaDoCaderno; onErro: (e: string) => void }) {
  const mudar = (m: Partial<MetaDoCaderno>) => void mudarCaderno(meta.id, m).catch((e) => onErro(textoDoErro(e)));
  return (
    <section className="caderno-ajustes" aria-label="Ajustes do caderno">
      <SegmentedControl label="Papel" value={meta.papel} onChange={(papel) => mudar({ papel })} options={(Object.keys(ROTULO_DO_PAPEL) as Papel[]).map((p) => ({ value: p, label: ROTULO_DO_PAPEL[p] }))} />
      <SegmentedControl label="Letra" value={meta.letra} onChange={(letra) => mudar({ letra })} options={(Object.keys(ROTULO_DA_LETRA) as Letra[]).map((l) => ({ value: l, label: ROTULO_DA_LETRA[l] }))} />
      <div className="flex flex-wrap gap-1.5" role="radiogroup" aria-label="Cor da capa">
        {CAPAS.map((c) => <button key={c} type="button" role="radio" aria-checked={meta.capa === c} aria-label={`Capa ${c}`} onClick={() => mudar({ capa: c })} className={`caderno-amostra is-pequena${meta.capa === c ? " is-escolhida" : ""}`} style={{ background: c }} />)}
      </div>
      {meta.tipo === "livro" && (
        <label className="block space-y-1 text-xs text-text-secondary"><span>Autor</span>
          <Input defaultValue={meta.autor} onBlur={(e) => { if (e.target.value.trim() !== meta.autor) mudar({ autor: e.target.value.trim().slice(0, 120) }); }} />
        </label>
      )}
      <EscolhaDaIa valor={meta.ia} onChange={(ia) => mudar({ ia })} />
    </section>
  );
}

function Folha({ meta, paginaId, onErro, onApagar }: { meta: MetaDoCaderno; paginaId: string; onErro: (e: string) => void; onApagar: () => void }) {
  useCadernos();
  const id = meta.id;
  const pagina = meta.paginas.find((p) => p.id === paginaId);
  const texto = textoDaPagina(id, paginaId);
  const [modo, setModo] = useState<"escrever" | "ler">("escrever");
  const [estado, setEstado] = useState<"" | "salvando" | "salvo">("");
  const [titulo, setTitulo] = useState(pagina?.titulo ?? "");
  const pendente = useRef<number | null>(null);
  const sujo = useRef(false);

  useEffect(() => { if (texto === undefined) void lerPagina(id, paginaId).catch((e) => onErro(textoDoErro(e))); }, [id, paginaId, texto, onErro]);
  // Página vazia abre já escrevendo; com texto, também (é um caderno). "Ler" mostra o Markdown formatado.

  const gravarAgora = () => {
    if (pendente.current !== null) { window.clearTimeout(pendente.current); pendente.current = null; }
    if (!sujo.current) return;
    sujo.current = false;
    setEstado("salvando");
    void gravarPagina(id, paginaId).then(() => setEstado("salvo")).catch((e) => { sujo.current = true; setEstado(""); onErro(textoDoErro(e)); });
  };
  const gravarRef = useRef(gravarAgora);
  gravarRef.current = gravarAgora;
  useEffect(() => () => gravarRef.current(), []);

  const mudar = (valor: string) => {
    digitarNaPagina(id, paginaId, valor);
    sujo.current = true;
    setEstado("");
    if (pendente.current !== null) window.clearTimeout(pendente.current);
    pendente.current = window.setTimeout(gravarAgora, ESPERA_PARA_GRAVAR_MS);
  };

  const salvarTitulo = () => { if (pagina && titulo.trim() !== pagina.titulo) void renomearPagina(id, paginaId, titulo.trim()).catch((e) => onErro(textoDoErro(e))); };
  const data = pagina ? new Date(pagina.criadaEm).toLocaleDateString("pt-BR", { day: "numeric", month: "long", year: "numeric" }) : "";
  const palavras = (texto ?? "").trim() ? (texto ?? "").trim().split(/\s+/).length : 0;

  return (
    <div className="caderno-folha-area">
      <div className="caderno-barra">
        <SegmentedControl value={modo} onChange={setModo} grow={false} options={[{ value: "escrever", label: "Escrever" }, { value: "ler", label: "Ler" }]} />
        <span className="ml-auto text-[11px] text-text-muted" role="status">
          {estado === "salvando" ? "Salvando…" : estado === "salvo" ? "Salvo" : ""}{palavras ? ` · ${palavras} ${palavras === 1 ? "palavra" : "palavras"}` : ""}
        </span>
        <button type="button" className="caderno-mini" onClick={onApagar} aria-label="Mandar página para a lixeira"><TrashIcon size={13} /></button>
      </div>
      <article className={`caderno-folha papel-${meta.papel} letra-${meta.letra}`}>
        <span className="caderno-espiral" aria-hidden="true" />
        <header className="caderno-folha-topo">
          <input className="caderno-folha-titulo" value={titulo} onChange={(e) => setTitulo(e.target.value)} onBlur={salvarTitulo}
            onKeyDown={(e) => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} placeholder="Título da página" aria-label="Título da página" />
          <time className="caderno-folha-data">{data}</time>
        </header>
        {texto === undefined ? <p className="caderno-folha-texto">Abrindo…</p>
          : modo === "escrever" ? (
            <textarea className="caderno-folha-texto" value={texto} onChange={(e) => mudar(e.target.value)} onBlur={gravarAgora} spellCheck autoFocus={!texto}
              aria-label="Texto da página" placeholder={meta.tipo === "livro" ? "O que você leu, o que marcou, o que pensou…\n\n## para títulos · - para listas · > para citações" : "Escreva aqui…\n\n## para títulos · - para listas · > para citações"} />
          ) : (
            <div className="caderno-folha-texto caderno-markdown">
              {texto.trim() ? <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeSanitize]}>{texto}</ReactMarkdown> : <p className="opacity-60">Página em branco.</p>}
            </div>
          )}
      </article>
    </div>
  );
}
