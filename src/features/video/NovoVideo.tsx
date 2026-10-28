// Fork (Editor de vídeo): criar um vídeo — briefing, referências (arquivos, links e pastas) e a IA.
// Vira uma pasta com BRIEF.md, assets/, midias.json e a composição inicial.
import { useEffect, useRef, useState } from "react";
import { Button, SegmentedControl } from "../../components/ui";
import { TrashIcon } from "../../components/ui/PixelIcon";
import * as disco from "./armazenamento";
import { IconeDaMidia } from "./Biblioteca";
import { escolherPastaDoComputador, nomeDaPasta } from "./PastaDoComputador";
import { normalizarMidias } from "./useProjeto";
import { BRIEFING_VAZIO, FORMATOS, gerarBrief, indexInicial, instrucoesDoAgente, secaoDeReferencias, atualizarReferenciasNoBrief } from "./modelos";
import { escolherArquivos, useSoltarArquivos } from "./soltarArquivos";
import type { Briefing, Formato, LinkDeReferencia, Midias, Papel, PastaDeReferencia } from "./tipos";

interface ArquivoPendente { caminho: string; papel: Papel }

function nomeDoCaminho(c: string) { return c.split(/[\\/]/).pop() ?? c; }
function tipoDoCaminho(c: string) {
  const e = c.split(".").pop()?.toLowerCase() ?? "";
  if (["mp4", "mov", "webm", "m4v", "mkv"].includes(e)) return "video";
  if (["png", "jpg", "jpeg", "gif", "webp", "svg", "avif", "heic"].includes(e)) return "imagem";
  if (["mp3", "wav", "m4a", "aac", "ogg", "flac"].includes(e)) return "audio";
  if (["ttf", "otf", "woff", "woff2"].includes(e)) return "fonte";
  return "outro";
}

export function NovoVideo({ onCriado, onCancelar, pastaInicial, computadorInicial }: { onCriado: (id: string, aviso?: string) => void; onCancelar: () => void; pastaInicial?: string; computadorInicial?: string }) {
  const [nome, setNome] = useState("");
  const [b, setB] = useState<Briefing>(BRIEFING_VAZIO);
  // A IA se escolhe no chat do editor (conversa normal do app). Aqui, a pasta do computador.
  const [computador, setComputador] = useState<string | null>(computadorInicial ?? null);
  const [arquivos, setArquivos] = useState<ArquivoPendente[]>([]);
  const [papel, setPapel] = useState<Papel>("referencia");
  const [links, setLinks] = useState<LinkDeReferencia[]>([]);
  const [url, setUrl] = useState("");
  const [pastas, setPastas] = useState<PastaDeReferencia[]>([]);
  const [escolhidas, setEscolhidas] = useState<string[]>(pastaInicial ? [pastaInicial] : []);
  const [criando, setCriando] = useState<string | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const area = useRef<HTMLDivElement>(null);
  const soltando = useSoltarArquivos(area, (c) => adicionar(c));

  useEffect(() => { void disco.listarPastas().then(setPastas).catch(() => undefined); }, []);

  function adicionar(caminhos: string[]) {
    setArquivos((l) => [...l, ...caminhos.filter((c) => !l.some((x) => x.caminho === c)).map((caminho) => ({ caminho, papel }))]);
  }
  const mudar = (parcial: Partial<Briefing>) => setB((x) => ({ ...x, ...parcial }));
  const adicionarLink = () => {
    const u = url.trim();
    if (!/^https?:\/\//i.test(u)) { setErro("O link precisa começar com http:// ou https://"); return; }
    if (!links.some((l) => l.url === u)) setLinks((l) => [...l, { url: u, nota: "" }]);
    setUrl(""); setErro(null);
  };

  const criar = async () => {
    if (!nome.trim()) { setErro("Dê um nome ao vídeo."); return; }
    setErro(null);
    setCriando("Criando a pasta do projeto…");
    const { largura, altura } = FORMATOS[b.formato];
    const daPasta = pastas.filter((p) => escolhidas.includes(p.id));
    try {
      const id = await disco.criarProjeto({
        projeto: { nome: nome.trim(), formato: b.formato, largura, altura, fps: 30, cor: "#7c9cff", briefing: b, pastasDeReferencia: escolhidas, ...(computador ? { pastaDoComputador: computador } : {}) },
        brief: gerarBrief(nome, b, [], links, daPasta, computador ?? undefined),
        index: indexInicial({ nome, formato: b.formato, duracao: b.duracao }),
        instrucoes: instrucoesDoAgente(),
      });
      const falhas: string[] = [];
      for (const [i, a] of arquivos.entries()) {
        setCriando(`Copiando referências (${i + 1} de ${arquivos.length})…`);
        try { await disco.importarMidia(id, a.caminho, a.papel); } catch (e) { falhas.push(`${nomeDoCaminho(a.caminho)}: ${String(e)}`); }
      }
      // As pastas (do computador e de referências) ficam ligadas, sem copiar: o agente lê de lá.
      setCriando("Ligando as pastas ao agente…");
      try { await disco.configurarPastas(id, computador, escolhidas); } catch (e) { falhas.push(`pastas: ${String(e)}`); }
      // Links e a seção de referências do BRIEF.md com tudo o que entrou.
      const m: Midias = normalizarMidias(JSON.parse((await disco.lerArquivo(id, "midias.json")) ?? "{}"));
      const todos = [...m.links, ...links.filter((l) => !m.links.some((x) => x.url === l.url))];
      await disco.gravarArquivo(id, "midias.json", JSON.stringify({ ...m, links: todos }, null, 2));
      const brief = (await disco.lerArquivo(id, "BRIEF.md")) ?? "";
      await disco.gravarArquivo(id, "BRIEF.md", atualizarReferenciasNoBrief(brief, secaoDeReferencias(m.itens, todos, daPasta, computador ?? undefined)));
      onCriado(id, falhas.length ? `Criei o vídeo, mas não copiei: ${falhas.join(" · ")}` : undefined);
    } catch (e) {
      setErro(e instanceof Error ? e.message : String(e));
      setCriando(null);
    }
  };

  return (
    <div className="video-novo">
      <header className="video-novo-topo">
        <h1>Novo vídeo</h1>
        <span className="flex-1" />
        <Button size="sm" variant="ghost" onClick={onCancelar} disabled={!!criando}>Cancelar</Button>
        <Button size="sm" variant="primary" onClick={() => void criar()} loading={!!criando} disabled={!!criando}>Criar e abrir o editor</Button>
      </header>
      <div className="video-novo-corpo">
        <section className="video-novo-coluna">
          <label className="video-campo"><span>Nome</span>
            <input className="field px-3 py-2 text-sm" value={nome} autoFocus placeholder="Ex.: Lançamento do app" onChange={(e) => setNome(e.target.value)} />
          </label>
          {/* Fork (0.23): a pasta vem logo no começo — é dela que a produção parte (antes ficava no fim). */}
          <div className="video-campo"><span>Pasta do computador</span>
            <p className="video-dica">Ex.: a pasta da sua marca, com identidade visual, logos, fontes e informações. O agente lê essa pasta para criar o vídeo; nada dela é apagado.</p>
            <div className="video-link-novo is-linha">
              <Button size="sm" variant="glass" onClick={() => void escolherPastaDoComputador().then((c) => { if (c) { setComputador(c); setErro(null); } }).catch((e) => setErro(String(e)))}>
                {computador ? `Trocar (${nomeDaPasta(computador)})` : "Escolher pasta…"}
              </Button>
              {computador && <Button size="sm" variant="ghost" onClick={() => setComputador(null)}>Sem pasta</Button>}
            </div>
            {computador && <p className="video-dica" title={computador}>{computador}</p>}
          </div>
          <div className="video-campo"><span>Formato</span>
            <div className="video-formatos" role="radiogroup" aria-label="Formato">
              {(Object.keys(FORMATOS) as Formato[]).map((f) => {
                const { largura, altura, dica } = FORMATOS[f];
                return (
                  <button key={f} type="button" role="radio" aria-checked={b.formato === f} className={`video-formato${b.formato === f ? " is-ativo" : ""}`} onClick={() => mudar({ formato: f })}>
                    <span className="video-formato-caixa" style={{ aspectRatio: `${largura} / ${altura}` }} aria-hidden="true" />
                    <b>{f}</b><small>{dica}</small>
                  </button>
                );
              })}
            </div>
          </div>
          <label className="video-campo"><span>Duração (segundos)</span>
            <input className="field px-3 py-2 text-sm" type="number" min={1} max={600} value={b.duracao} onChange={(e) => mudar({ duracao: Math.max(1, Math.min(600, Number(e.target.value) || 1)) })} />
          </label>
          <label className="video-campo"><span>O que você quer</span>
            <textarea className="field px-3 py-2 text-sm" rows={5} value={b.descricao} placeholder="Ex.: vídeo de 15 s mostrando as três telas novas, com texto grande e música animada no fim." onChange={(e) => mudar({ descricao: e.target.value })} />
          </label>
          <label className="video-campo"><span>Estilo</span>
            <input className="field px-3 py-2 text-sm" value={b.estilo} placeholder="Ex.: minimalista, cortes rápidos, cores da marca" onChange={(e) => mudar({ estilo: e.target.value })} />
          </label>
          <div className="grid grid-cols-2 gap-3">
            <label className="video-campo"><span>Público</span>
              <input className="field px-3 py-2 text-sm" value={b.publico} placeholder="Ex.: donos de clínica" onChange={(e) => mudar({ publico: e.target.value })} />
            </label>
            <label className="video-campo"><span>Onde vai passar</span>
              <input className="field px-3 py-2 text-sm" value={b.plataforma} placeholder="Ex.: Reels, YouTube" onChange={(e) => mudar({ plataforma: e.target.value })} />
            </label>
          </div>
          <label className="video-campo"><span>Observações</span>
            <textarea className="field px-3 py-2 text-sm" rows={2} value={b.observacoes} placeholder="Ex.: sem narração; logo no fim por 2 s" onChange={(e) => mudar({ observacoes: e.target.value })} />
          </label>
        </section>

        <section className="video-novo-coluna">
          <div ref={area} className={`video-solte is-grande${soltando ? " is-soltando" : ""}`}>
            <p><b>Arraste as referências aqui</b><br />vídeos, fotos, áudio, fontes, logos</p>
            <SegmentedControl<Papel> value={papel} onChange={setPapel} grow={false}
              options={[{ value: "referencia", label: "Só referência", title: "O agente olha o estilo" }, { value: "usar", label: "Usar no vídeo" }]} />
            <Button size="sm" variant="glass" onClick={() => void escolherArquivos().then(adicionar)}>Escolher arquivos…</Button>
          </div>
          {arquivos.length > 0 && (
            <ul className="video-bib-lista">
              {arquivos.map((a) => (
                <li key={a.caminho} className="video-bib-item">
                  <span className="video-mini is-icone"><IconeDaMidia tipo={tipoDoCaminho(a.caminho)} /></span>
                  <span className="video-bib-nome" title={a.caminho}>{nomeDoCaminho(a.caminho)}</span>
                  <button type="button" className={`video-papel is-${a.papel}`} onClick={() => setArquivos((l) => l.map((x) => (x.caminho === a.caminho ? { ...x, papel: x.papel === "usar" ? "referencia" : "usar" } : x)))}>
                    {a.papel === "usar" ? "usar" : "referência"}
                  </button>
                  <button type="button" className="video-mini-botao" aria-label={`Tirar ${nomeDoCaminho(a.caminho)}`} onClick={() => setArquivos((l) => l.filter((x) => x.caminho !== a.caminho))}><TrashIcon size={12} /></button>
                </li>
              ))}
            </ul>
          )}
          <div className="video-campo"><span>Links de referência</span>
            <div className="video-link-novo is-linha">
              <input className="field px-2.5 py-1.5 text-xs" placeholder="https://…" value={url} onChange={(e) => setUrl(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); adicionarLink(); } }} />
              <Button size="sm" variant="glass" disabled={!url.trim()} onClick={adicionarLink}>Adicionar</Button>
            </div>
            <ul className="video-links">{links.map((l) => (
              <li key={l.url}><span>{l.url.replace(/^https?:\/\//, "")}</span>
                <button type="button" className="video-mini-botao" aria-label={`Tirar ${l.url}`} onClick={() => setLinks((x) => x.filter((y) => y.url !== l.url))}><TrashIcon size={12} /></button>
              </li>
            ))}</ul>
          </div>
          <div className="video-campo"><span>Pastas de referências</span>
            <p className="video-dica">Escolha as que valem para este vídeo. Vídeos da mesma pasta do computador podem usar referências diferentes.</p>
            {pastas.length === 0 && <p className="video-vazio">Nenhuma pasta ainda. Crie uma na tela de vídeos para reaproveitar referências de um tipo de vídeo.</p>}
            <div className="video-pastas-escolha">
              {pastas.map((p) => (
                <button key={p.id} type="button" aria-pressed={escolhidas.includes(p.id)} className={`video-pasta-chip${escolhidas.includes(p.id) ? " is-ativa" : ""}`}
                  onClick={() => setEscolhidas((l) => (l.includes(p.id) ? l.filter((x) => x !== p.id) : [...l, p.id]))} title={p.descricao}>
                  <span className="video-pasta-cor" style={{ background: p.cor }} aria-hidden="true" />{p.nome}<small>{p.itens.length}</small>
                </button>
              ))}
            </div>
          </div>
          {criando && <p className="video-andamento-linha" role="status">{criando}</p>}
          {erro && <p className="video-erro" role="alert">{erro}</p>}
        </section>
      </div>
    </div>
  );
}
