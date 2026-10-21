// Fork (Roadmaps de estudo): a tela — lista de roadmaps, entrevista ou trilha do roadmap aberto.
import { useEffect, useState } from "react";
import { Button } from "../../components/ui";
import { lembrarUltimo, ultimoAberto } from "./armazenamento";
import { Entrevista } from "./Entrevista";
import { ListaDeRoadmaps } from "./ListaDeRoadmaps";
import { carregar, carregarLista, erroDaListaAtual, erroDoRoadmap, lembranca, lembrar, listaAtual, recarregar, roadmap, useLoja } from "./loja";
import { Trilha } from "./Trilha";
import "./roadmaps.css";

/** Roadmap aberto na última vez que a tela esteve aberta (nesta execução do app). */
const ABERTO = "aberto";

export function RoadmapsView({ onClose, continuar = false }: { onClose: () => void; continuar?: boolean }) {
  useLoja();
  const [aberto, setAberto] = useState<string | null>(() => (continuar ? lembranca<string>(ABERTO) ?? ultimoAberto() : null));
  const lista = listaAtual();

  useEffect(() => { void carregarLista(); }, []);
  useEffect(() => {
    lembrar(ABERTO, aberto ?? undefined);
    if (!aberto) return;
    lembrarUltimo(aberto);
    void carregar(aberto).catch(() => undefined);
  }, [aberto]);
  // Voltou para a janela: o celular pode ter mudado algo (cards, caderno, etapa concluída). Só evento, sem timer.
  useEffect(() => {
    const aoVoltar = () => {
      if (document.hidden) return;
      if (aberto) void recarregar(aberto); else void carregarLista();
    };
    window.addEventListener("focus", aoVoltar);
    document.addEventListener("visibilitychange", aoVoltar);
    return () => { window.removeEventListener("focus", aoVoltar); document.removeEventListener("visibilitychange", aoVoltar); };
  }, [aberto]);
  // "Continuar estudo" com um roadmap que não existe mais: cai na lista.
  useEffect(() => {
    if (aberto && lista && !lista.some((m) => m.id === aberto)) setAberto(null);
  }, [aberto, lista]);

  const voltar = () => { setAberto(null); void carregarLista(); };

  if (aberto) {
    const r = roadmap(aberto);
    const erro = erroDoRoadmap(aberto);
    if (erro && !r) return (
      <div className="p-6 space-y-3">
        <p role="alert" className="roadmap-erro">Não consegui abrir este roadmap: {erro}</p>
        <Button size="sm" onClick={voltar}>← Roadmaps</Button>
      </div>
    );
    if (!r) return <div className="p-6 font-mono text-xs text-text-muted">Abrindo roadmap…</div>;
    return r.meta.fase === "ativo" && r.plano ? <Trilha r={r} onVoltar={voltar} /> : <Entrevista r={r} onVoltar={voltar} />;
  }
  if (!lista) {
    const erro = erroDaListaAtual();
    return erro
      ? <div className="p-6"><p role="alert" className="roadmap-erro">Não consegui ler os roadmaps: {erro}</p></div>
      : <div className="p-6 font-mono text-xs text-text-muted">Abrindo roadmaps…</div>;
  }
  return <ListaDeRoadmaps lista={lista} onAbrir={setAberto} onFechar={onClose} />;
}
