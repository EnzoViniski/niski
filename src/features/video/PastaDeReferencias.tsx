// Fork (Editor de vídeo): editar uma pasta de referências (nome, estilo, cor, arquivos e links).
import { useEffect, useRef, useState } from "react";
import { Button, SegmentedControl } from "../../components/ui";
import { ArrowLeftIcon, LinkIcon, TrashIcon } from "../../components/ui/PixelIcon";
import * as disco from "./armazenamento";
import { Miniatura } from "./Biblioteca";
import { carregarEndereco, urlDaPasta } from "./enderecos";
import { urlSegura } from "../roadmaps/normalizar";
import { tamanhoLegivel } from "./midiaNoNavegador";
import { escolherArquivos, useSoltarArquivos } from "./soltarArquivos";
import type { Papel, PastaDeReferencia } from "./tipos";

export const CORES_DAS_PASTAS = ["#7c9cff", "#f472b6", "#34d399", "#fbbf24", "#f87171", "#a78bfa", "#22d3ee", "#fb923c"];

export function PastaDeReferencias({ id, onVoltar, onNovoVideo }: { id: string; onVoltar: () => void; onNovoVideo: (pasta: string) => void }) {
  const [pasta, setPasta] = useState<PastaDeReferencia | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState<string | null>(null);
  const [papel, setPapel] = useState<Papel>("referencia");
  const [url, setUrl] = useState("");
  const [, setToken] = useState(false);
  const area = useRef<HTMLDivElement>(null);
  const soltando = useSoltarArquivos(area, (c) => void importar(c));

  useEffect(() => {
    void carregarEndereco().then(() => setToken(true)).catch(() => undefined);
    void disco.lerPasta(id).then(setPasta).catch((e) => setErro(String(e)));
  }, [id]);

  async function importar(caminhos: string[]) {
    const falhas: string[] = [];
    for (const [i, c] of caminhos.entries()) {
      setOcupado(`Copiando ${i + 1} de ${caminhos.length}…`);
      try { await disco.importarNaPasta(id, c, papel); } catch (e) { falhas.push(`${c.split(/[\\/]/).pop()}: ${String(e)}`); }
    }
    setOcupado(null);
    setErro(falhas.length ? falhas.join(" · ") : null);
    setPasta(await disco.lerPasta(id));
  }
  const editar = async (dados: Partial<PastaDeReferencia>) => {
    try { setPasta(await disco.editarPasta(id, dados)); } catch (e) { setErro(String(e)); }
  };

  if (!pasta) return <div className="p-6 text-xs text-text-muted">{erro ?? "Abrindo a pasta…"}</div>;
  return (
    <div className="video-pasta-tela">
      <header className="video-novo-topo">
        <Button size="sm" variant="ghost" onClick={onVoltar}><ArrowLeftIcon size={14} /> Vídeos</Button>
        <input className="video-nome" defaultValue={pasta.nome} aria-label="Nome da pasta" key={pasta.nome}
          onBlur={(e) => { const n = e.target.value.trim(); if (n && n !== pasta.nome) void editar({ nome: n }); }} onKeyDown={(e) => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} />
        <span className="flex-1" />
        <Button size="sm" variant="primary" onClick={() => onNovoVideo(pasta.id)}>Novo vídeo com esta pasta</Button>
      </header>
      <div className="video-novo-corpo">
        <section className="video-novo-coluna">
          <label className="video-campo"><span>Estilo deste tipo de vídeo</span>
            <textarea className="field px-3 py-2 text-sm" rows={5} defaultValue={pasta.descricao} key={pasta.atualizadoEm}
              placeholder="Ex.: Reels da Medza: 9:16, cortes a cada 1,5 s, legenda grande amarela, logo no fim."
              onBlur={(e) => { if (e.target.value !== pasta.descricao) void editar({ descricao: e.target.value }); }} />
          </label>
          <div className="video-campo"><span>Cor</span>
            <div className="flex gap-1.5" role="radiogroup" aria-label="Cor da pasta">
              {CORES_DAS_PASTAS.map((c) => <button key={c} type="button" role="radio" aria-checked={pasta.cor === c} aria-label={`Cor ${c}`} className="video-cor" style={{ background: c, outlineColor: pasta.cor === c ? c : "transparent" }} onClick={() => void editar({ cor: c })} />)}
            </div>
          </div>
          <div className="video-campo"><span>Links</span>
            <div className="video-link-novo is-linha">
              <input className="field px-2.5 py-1.5 text-xs" placeholder="https://…" value={url} onChange={(e) => setUrl(e.target.value)}
                onKeyDown={(e) => { if (e.key === "Enter" && /^https?:\/\//i.test(url.trim())) { void editar({ links: [...pasta.links, { url: url.trim(), nota: "" }] }); setUrl(""); } }} />
              <Button size="sm" variant="glass" disabled={!/^https?:\/\//i.test(url.trim())} onClick={() => { void editar({ links: [...pasta.links, { url: url.trim(), nota: "" }] }); setUrl(""); }}>Adicionar</Button>
            </div>
            <ul className="video-links">{pasta.links.map((l) => (
              <li key={l.url}><LinkIcon size={12} aria-hidden="true" /><a href={urlSegura(l.url) ?? undefined}>{l.url.replace(/^https?:\/\//, "")}</a>
                <button type="button" className="video-mini-botao" aria-label={`Tirar ${l.url}`} onClick={() => void editar({ links: pasta.links.filter((x) => x.url !== l.url) })}><TrashIcon size={12} /></button>
              </li>
            ))}</ul>
          </div>
        </section>
        <section className="video-novo-coluna">
          <div ref={area} className={`video-solte is-grande${soltando ? " is-soltando" : ""}`}>
            <p><b>Arraste arquivos para esta pasta</b><br />vídeos, fotos, fontes, áudio, logos</p>
            <SegmentedControl<Papel> value={papel} onChange={setPapel} grow={false}
              options={[{ value: "referencia", label: "Só referência" }, { value: "usar", label: "Usar no vídeo" }]} />
            <Button size="sm" variant="glass" onClick={() => void escolherArquivos().then((c) => { if (c.length) void importar(c); })}>Escolher arquivos…</Button>
          </div>
          {ocupado && <p className="video-andamento-linha" role="status">{ocupado}</p>}
          {erro && <p className="video-erro" role="alert">{erro}</p>}
          <ul className="video-bib-lista">
            {pasta.itens.map((m) => (
              <li key={m.id} className="video-bib-item" title={`${m.nome} · ${tamanhoLegivel(m.tamanho)}`}>
                <Miniatura midia={m} url={urlDaPasta(pasta.id, m.arquivo)} />
                <span className="video-bib-nome">{m.nome}</span>
                <span className={`video-papel is-${m.papel}`}>{m.papel === "usar" ? "usar" : "referência"}</span>
                <button type="button" className="video-mini-botao" aria-label={`Tirar ${m.nome}`}
                  onClick={() => void disco.removerDaPasta(pasta.id, m.id).then(() => disco.lerPasta(id)).then(setPasta).catch((e) => setErro(String(e)))}><TrashIcon size={12} /></button>
              </li>
            ))}
          </ul>
          {pasta.itens.length === 0 && <p className="video-vazio">Pasta vazia.</p>}
        </section>
      </div>
    </div>
  );
}
