// Fork (Editor de vídeo): biblioteca do projeto — mídias (arrastar do Finder), links e pastas de
// referências, e o briefing (BRIEF.md) que o agente lê antes de editar.
import { useEffect, useRef, useState } from "react";
import { Button, SegmentedControl } from "../../components/ui";
import { AlertIcon, FileIcon, ImageIcon, LinkIcon, MusicIcon, NoteIcon, TrashIcon, VideoIcon } from "../../components/ui/PixelIcon";
import * as disco from "./armazenamento";
import { urlDoProjeto } from "./enderecos";
import { tamanhoLegivel } from "./midiaNoNavegador";
import { escolherArquivos, useSoltarArquivos } from "./soltarArquivos";
import { TIPO_DE_ARRASTE_DA_MIDIA } from "./Timeline";
import type { AcoesDoProjeto } from "./useProjeto";
import type { LinkDeReferencia, Midia, Papel, PastaDeReferencia } from "./tipos";

export type AbaDaBiblioteca = "propriedades" | "midia" | "referencias" | "briefing";
type Aba = AbaDaBiblioteca;

export function IconeDaMidia({ tipo, tamanho = 16 }: { tipo: string; tamanho?: number }) {
  const Icone = tipo === "video" ? VideoIcon : tipo === "imagem" ? ImageIcon : tipo === "audio" ? MusicIcon : tipo === "fonte" ? NoteIcon : FileIcon;
  return <Icone size={tamanho} aria-hidden="true" />;
}

/** Miniatura: imagem de verdade, primeiro quadro do vídeo, ou o ícone. */
export function Miniatura({ midia, url }: { midia: Pick<Midia, "tipo" | "nome">; url: string | null }) {
  const [falhou, setFalhou] = useState(false);
  if (url && !falhou && midia.tipo === "imagem") return <img className="video-mini" src={url} alt="" loading="lazy" onError={() => setFalhou(true)} />;
  if (url && !falhou && midia.tipo === "video") return <video className="video-mini" src={`${url}#t=0.1`} muted preload="metadata" playsInline onError={() => setFalhou(true)} />;
  return <span className="video-mini is-icone"><IconeDaMidia tipo={midia.tipo} tamanho={20} /></span>;
}

export function Biblioteca({ projeto, acoes, bloqueado, onPorNaTimeline, aba: abaDeFora, onAba, propriedades }: {
  projeto: string; acoes: AcoesDoProjeto; bloqueado: boolean; onPorNaTimeline: (m: Midia) => void;
  /** Fork (0.16): aba controlada pelo editor (escolher algo no vídeo abre "Propriedades"). */
  aba?: Aba; onAba?: (a: Aba) => void; propriedades?: React.ReactNode;
}) {
  const { estado } = acoes;
  const [abaPropria, setAbaPropria] = useState<Aba>("midia");
  const aba = abaDeFora ?? abaPropria;
  const setAba = onAba ?? setAbaPropria;
  const [papel, setPapel] = useState<Papel>("usar");
  const area = useRef<HTMLDivElement>(null);
  const arrastando = useSoltarArquivos(area, (caminhos) => { if (!bloqueado) void acoes.importar(caminhos, papel); }, aba === "midia");

  return (
    <div className="video-bib">
      <div className="video-abas" role="tablist" aria-label="Biblioteca">
        {([...(propriedades ? [["propriedades", "Propriedades"]] as const : []), ["midia", "Mídia"], ["referencias", "Referências"], ["briefing", "Briefing"]] as const).map(([v, r]) => (
          <button key={v} type="button" role="tab" aria-selected={aba === v} className={`video-aba${aba === v ? " is-ativa" : ""}`} onClick={() => setAba(v)}>{r}</button>
        ))}
      </div>
      {aba === "midia" && (
        <div ref={area} className={`video-bib-corpo${arrastando ? " is-soltando" : ""}`}>
          {estado.faltando.length > 0 && (
            <p className="video-aviso" role="alert"><AlertIcon size={14} aria-hidden="true" />
              {estado.faltando.length === 1 ? `“${estado.faltando[0].nome}” ${estado.faltando[0].motivo === "sumiu" ? "sumiu da pasta" : "mudou no disco"}.` : `${estado.faltando.length} mídias sumiram ou mudaram.`} Importe de novo para o vídeo usar.
            </p>
          )}
          <div className="video-solte">
            <p>Arraste vídeos, fotos, áudio, fontes e logos aqui</p>
            <SegmentedControl<Papel> value={papel} onChange={setPapel} grow={false}
              options={[{ value: "usar", label: "Usar no vídeo" }, { value: "referencia", label: "Só referência", title: "O agente olha o estilo, mas não coloca no vídeo" }]} />
            <Button size="sm" variant="glass" disabled={bloqueado} onClick={() => void escolherArquivos().then((c) => { if (c.length) void acoes.importar(c, papel); })}>Escolher arquivos…</Button>
          </div>
          {estado.midias.itens.length === 0 && <p className="video-vazio">Nada importado ainda.</p>}
          <ul className="video-bib-lista">
            {estado.midias.itens.map((m) => {
              const falta = estado.faltando.some((f) => f.id === m.id);
              return (
                <li key={m.id} className={`video-bib-item${falta ? " is-faltando" : ""}`} draggable={!falta && !bloqueado}
                  onDragStart={(e) => { e.dataTransfer.setData(TIPO_DE_ARRASTE_DA_MIDIA, m.id); e.dataTransfer.effectAllowed = "copy"; }}
                  title={`${m.nome} · ${tamanhoLegivel(m.tamanho)}${m.nota ? `\n${m.nota}` : ""}`}>
                  <Miniatura midia={m} url={falta ? null : urlDoProjeto(projeto, m.arquivo)} />
                  <span className="video-bib-nome">{m.nome}</span>
                  <button type="button" className={`video-papel is-${m.papel}`} disabled={bloqueado}
                    onClick={() => void acoes.mudarMidia(m.id, { papel: m.papel === "usar" ? "referencia" : "usar" })}
                    title="Trocar entre usar no vídeo e só referência">{m.papel === "usar" ? "usar" : "referência"}</button>
                  <span className="video-bib-acoes">
                    {["video", "imagem", "audio"].includes(m.tipo) && <button type="button" className="video-mini-botao" disabled={bloqueado || falta} onClick={() => onPorNaTimeline(m)} title="Pôr na timeline, na agulha">＋</button>}
                    <button type="button" className="video-mini-botao" disabled={bloqueado} onClick={() => void acoes.removerMidia(m.id)} title="Tirar da biblioteca (vai para a lixeira do app)" aria-label={`Remover ${m.nome}`}><TrashIcon size={12} /></button>
                  </span>
                </li>
              );
            })}
          </ul>
        </div>
      )}
      {aba === "propriedades" && propriedades && <div className="video-bib-corpo">{propriedades}</div>}
      {aba === "referencias" && <AbaDeReferencias projeto={projeto} acoes={acoes} bloqueado={bloqueado} />}
      {aba === "briefing" && <AbaDoBriefing acoes={acoes} bloqueado={bloqueado} />}
    </div>
  );
}

function AbaDeReferencias({ projeto, acoes, bloqueado }: { projeto: string; acoes: AcoesDoProjeto; bloqueado: boolean }) {
  const { estado } = acoes;
  const [url, setUrl] = useState("");
  const [nota, setNota] = useState("");
  const [pastas, setPastas] = useState<PastaDeReferencia[]>([]);
  const [salvarComo, setSalvarComo] = useState("");
  const [aviso, setAviso] = useState<string | null>(null);
  useEffect(() => { void disco.listarPastas().then(setPastas).catch(() => undefined); }, []);
  const links = estado.midias.links;
  const adicionar = () => {
    const u = url.trim();
    if (!/^https?:\/\//i.test(u)) { setAviso("O link precisa começar com http:// ou https://"); return; }
    if (links.some((l) => l.url === u)) { setUrl(""); return; }
    void acoes.mudarLinks([...links, { url: u, nota: nota.trim() }]);
    setUrl(""); setNota(""); setAviso(null);
  };
  const escolhidas = estado.projeto?.pastasDeReferencia ?? [];
  return (
    <div className="video-bib-corpo">
      <h4 className="video-subtitulo">Links de referência</h4>
      <div className="video-link-novo">
        <input className="field px-2.5 py-1.5 text-xs" placeholder="https://… (vídeo, post, site)" value={url} disabled={bloqueado}
          onChange={(e) => setUrl(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") adicionar(); e.stopPropagation(); }} />
        <input className="field px-2.5 py-1.5 text-xs" placeholder="O que olhar nele (opcional)" value={nota} disabled={bloqueado}
          onChange={(e) => setNota(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") adicionar(); e.stopPropagation(); }} />
        <Button size="sm" variant="glass" disabled={bloqueado || !url.trim()} onClick={adicionar}>Adicionar link</Button>
      </div>
      {aviso && <p className="video-aviso">{aviso}</p>}
      <ul className="video-links">
        {links.map((l: LinkDeReferencia) => (
          <li key={l.url}><LinkIcon size={12} aria-hidden="true" /><a href={l.url} title={l.url}>{l.url.replace(/^https?:\/\//, "")}</a>{l.nota && <small>{l.nota}</small>}
            <button type="button" className="video-mini-botao" disabled={bloqueado} aria-label={`Tirar ${l.url}`} onClick={() => void acoes.mudarLinks(links.filter((x) => x.url !== l.url))}><TrashIcon size={12} /></button>
          </li>
        ))}
      </ul>
      <h4 className="video-subtitulo">Pastas de referências</h4>
      <p className="video-dica">Ligue as pastas que valem para este vídeo: o agente lê os arquivos e links delas. “Copiar” traz os arquivos para a biblioteca, para arrastar na timeline.</p>
      {pastas.length === 0 && <p className="video-vazio">Nenhuma pasta ainda. Crie na tela de vídeos ou salve as mídias deste projeto como pasta.</p>}
      <ul className="video-pastas-mini">
        {pastas.map((p) => (
          <li key={p.id}>
            <span className="video-pasta-cor" style={{ background: p.cor }} aria-hidden="true" />
            <span className="video-bib-nome">{p.nome} <small>{p.itens.length} arquivo{p.itens.length === 1 ? "" : "s"}{p.links.length ? ` · ${p.links.length} link${p.links.length === 1 ? "" : "s"}` : ""}</small></span>
            {/* Fork: ligar/desligar por vídeo (sem copiar); copiar ficou opcional. */}
            <button type="button" role="switch" aria-checked={escolhidas.includes(p.id)} disabled={bloqueado}
              className={`video-papel ${escolhidas.includes(p.id) ? "is-usar" : "is-referencia"}`}
              title={escolhidas.includes(p.id) ? "Ligada: o agente usa esta pasta neste vídeo" : "Desligada neste vídeo"}
              onClick={() => void acoes.configurarPastas({ pastasDeReferencia: escolhidas.includes(p.id) ? escolhidas.filter((x) => x !== p.id) : [...escolhidas, p.id] })}>
              {escolhidas.includes(p.id) ? "ligada" : "desligada"}
            </button>
            {p.itens.length > 0 && <Button size="sm" variant="ghost" disabled={bloqueado} onClick={() => void acoes.aplicarPasta(p)} title="Copiar os arquivos desta pasta para a biblioteca do vídeo">Copiar</Button>}
          </li>
        ))}
      </ul>
      <div className="video-link-novo">
        <input className="field px-2.5 py-1.5 text-xs" placeholder="Nome da pasta nova (ex.: Reels da Medza)" value={salvarComo}
          onChange={(e) => setSalvarComo(e.target.value)} onKeyDown={(e) => e.stopPropagation()} />
        <Button size="sm" variant="glass" disabled={!salvarComo.trim() || estado.midias.itens.length + links.length === 0}
          onClick={() => void disco.pastaDoProjeto(projeto, { nome: salvarComo.trim(), descricao: estado.projeto?.briefing?.estilo ?? "" })
            .then((p) => { setPastas((l) => [p, ...l]); setSalvarComo(""); setAviso(`Pasta “${p.nome}” criada com ${p.itens.length} arquivo(s).`); })
            .catch((e) => setAviso(String(e)))}>
          Salvar mídias como pasta
        </Button>
      </div>
    </div>
  );
}

function AbaDoBriefing({ acoes, bloqueado }: { acoes: AcoesDoProjeto; bloqueado: boolean }) {
  const { estado } = acoes;
  const [texto, setTexto] = useState(estado.brief);
  const editando = useRef(false);
  useEffect(() => { if (!editando.current) setTexto(estado.brief); }, [estado.brief]);
  return (
    <div className="video-bib-corpo">
      <p className="video-dica">O agente lê este briefing antes de cada pedido. A seção de mídias e referências se atualiza sozinha.</p>
      <textarea className="field video-brief" value={texto} disabled={bloqueado} spellCheck
        onFocus={() => { editando.current = true; }}
        onChange={(e) => setTexto(e.target.value)}
        onBlur={() => { editando.current = false; void acoes.gravarBrief(texto); }}
        onKeyDown={(e) => e.stopPropagation()} aria-label="Briefing do vídeo" />
    </div>
  );
}
