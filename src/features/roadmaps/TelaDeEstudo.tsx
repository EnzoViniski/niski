// Fork (Roadmaps): uma etapa em tela cheia para estudar — conteúdo à esquerda; fonte (navegador dentro
// do roadmap), caderno e IA à direita. A lateral do app some enquanto esta tela está aberta.
import { useEffect, useState } from "react";
import { Button } from "../../components/ui";
import { definirEstudoEmTelaCheia } from "../../lib/modoFoco";
import { Caderno } from "./Caderno";
import { BotaoDeFoco, ChipDaIa, IconeDoRoadmap } from "./comuns";
import { lembranca, lembrar, useLoja } from "./loja";
import { NavegadorDoEstudo } from "./NavegadorDoEstudo";
import { Divisor, TamanhoDoTexto, useEscalaDoTexto, useTamanhoSalvo } from "./Divisor";
import { useTransicao } from "./useTransicao";
import { PainelDaEtapa } from "./PainelDaEtapa";
import { PainelDaIa } from "./PainelDaIa";
import { etapasEmOrdem } from "./progresso";
import type { RoadmapAberto } from "./armazenamento";
import type { EstadoDaEtapa } from "./tipos";

type Aba = "fonte" | "caderno" | "ia";

export function TelaDeEstudo({ r, etapaId, estado, url, onUrl, onSair, onAbrirEtapa }: {
  r: RoadmapAberto; etapaId: string; estado: EstadoDaEtapa; url: string | null;
  onUrl: (url: string | null) => void; onSair: () => void; onAbrirEtapa: (id: string) => void;
}) {
  useLoja();
  const id = r.meta.id;
  const etapa = r.plano ? etapasEmOrdem(r.plano).find((e) => e.id === etapaId) : undefined;
  const [aba, setAba] = useState<Aba>(() => lembranca<Aba>(`aba-do-estudo:${id}`) ?? (url ? "fonte" : etapa?.fontes.length ? "fonte" : "caderno"));
  const [chipAberto, setChipAberto] = useState(false);
  const [titulo, setTitulo] = useState("");
  const [larguraDoConteudo, setLarguraDoConteudo] = useTamanhoSalvo("estudo-conteudo", 520, 300, 1100);
  const [escala, setEscala] = useEscalaDoTexto();
  const raiz = useTransicao<HTMLDivElement>(`estudo:${etapaId}`, "frente");
  const conteudoDaAba = useTransicao<HTMLDivElement>(aba, "neutra", false);

  useEffect(() => { definirEstudoEmTelaCheia(true); return () => definirEstudoEmTelaCheia(false); }, []);
  useEffect(() => { lembrar(`aba-do-estudo:${id}`, aba); }, [id, aba]);
  useEffect(() => { if (url) setAba("fonte"); }, [url]);
  // Esc sai da tela cheia (fora de campos de texto).
  useEffect(() => {
    const tecla = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || chipAberto) return;
      if (e.target instanceof HTMLElement && e.target.closest("input, textarea, [contenteditable], [role=dialog]")) return;
      onSair();
    };
    window.addEventListener("keydown", tecla);
    return () => window.removeEventListener("keydown", tecla);
  }, [onSair, chipAberto]);

  const abrirLink = (novo: string) => { onUrl(novo); setAba("fonte"); };

  return (
    <div ref={raiz} className="roadmap-estudo" style={{ ["--roadmap-cor" as string]: r.meta.cor, ["--roadmap-escala" as string]: escala }}>
      <header className="roadmap-topo">
        <Button size="sm" variant="ghost" onClick={onSair} title="Voltar à trilha (Esc)">← Trilha</Button>
        <IconeDoRoadmap icone={r.meta.icone} cor={r.meta.cor} tamanho={28} />
        <div className="min-w-0 flex-1">
          <p className="truncate text-xs text-text-muted">{r.meta.nome}</p>
          <h1 className="truncate text-base font-semibold text-text-primary">{etapa?.titulo ?? "Etapa"}</h1>
        </div>
        <TamanhoDoTexto escala={escala} onChange={setEscala} />
        <ChipDaIa meta={r.meta} onAberto={setChipAberto} />
        <BotaoDeFoco />
      </header>
      <div className="roadmap-estudo-corpo" style={{ gridTemplateColumns: `${larguraDoConteudo}px auto minmax(0, 1fr)` }}>
        <div className="roadmap-estudo-conteudo">
          <PainelDaEtapa r={r} etapaId={etapaId} estado={estado} emTelaCheia onAbrirLink={abrirLink}
            onPerguntar={() => setAba("ia")} onAbrirEtapa={onAbrirEtapa} />
        </div>
        <Divisor valor={larguraDoConteudo} min={300} max={1100} onChange={setLarguraDoConteudo} cresce="direita" rotulo="Largura do conteúdo da etapa" />
        <section className="roadmap-estudo-lado" aria-label="Fonte, caderno e IA">
          <div className="roadmap-abas" role="tablist">
            <button role="tab" aria-selected={aba === "fonte"} onClick={() => setAba("fonte")}>Fonte</button>
            <button role="tab" aria-selected={aba === "caderno"} onClick={() => setAba("caderno")}>Caderno</button>
            <button role="tab" aria-selected={aba === "ia"} onClick={() => setAba("ia")}>IA</button>
          </div>
          <div ref={conteudoDaAba} className="roadmap-estudo-aba-conteudo">
          {aba === "fonte" && (
            <div className="roadmap-estudo-fonte">
              {etapa && etapa.fontes.length > 0 && (
                <div className="roadmap-fontes-chips" role="group" aria-label="Fontes da etapa">
                  {etapa.fontes.map((f) => (
                    <button key={f.url} className={`roadmap-chip ${f.url === url ? "is-ativo" : ""}`} onClick={() => abrirLink(f.url)} title={f.url}>
                      {f.titulo}<span className={`roadmap-selo ${f.gratis ? "is-gratis" : "is-pago"}`}>{f.gratis ? "Grátis" : "Pago"}</span>
                    </button>
                  ))}
                  {url && <Button size="sm" variant="ghost" onClick={() => onUrl(null)}>Fechar página</Button>}
                </div>
              )}
              {url ? (
                <div className="min-h-0 flex-1" aria-label={titulo ? `Página: ${titulo}` : "Página"}>
                  <NavegadorDoEstudo roadmapId={id} url={url} oculto={chipAberto} onTitulo={setTitulo} />
                </div>
              ) : (
                <p className="p-6 text-center text-sm text-text-muted">Escolha uma fonte acima ou clique num link do conteúdo: a página abre aqui, do lado do que você está estudando.</p>
              )}
            </div>
          )}
          {aba === "caderno" && <div className="roadmap-estudo-aba"><Caderno r={r} etapa={etapa} onAbrirLink={abrirLink} /></div>}
          {aba === "ia" && <PainelDaIa r={r} etapaId={etapaId} onAbrirLink={abrirLink} />}
          </div>
        </section>
      </div>
    </div>
  );
}
