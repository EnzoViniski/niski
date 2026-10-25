// Fork (Caderno): a parte "Caderno" dos Roadmaps — a estante ou o caderno aberto (lembra o último).
import { useEffect, useState } from "react";
import { CadernoAberto } from "./CadernoAberto";
import { Estante } from "./Estante";
import { caderno, cadernos, carregarCadernos, erroDosCadernos, gravacoesPendentes, useCadernos } from "./loja";
import "./caderno.css";

const ULTIMO = "medza-caderno-aberto";
function lerUltimo() { try { return localStorage.getItem(ULTIMO); } catch { return null; } }
function gravarUltimo(id: string | null) { try { if (id) localStorage.setItem(ULTIMO, id); else localStorage.removeItem(ULTIMO); } catch { /* sem localStorage */ } }

export function CadernoView({ continuar = true }: { continuar?: boolean }) {
  useCadernos();
  const [aberto, setAberto] = useState<string | null>(() => (continuar ? lerUltimo() : null));
  const lista = cadernos();

  useEffect(() => { void carregarCadernos(); }, []);
  useEffect(() => { gravarUltimo(aberto); }, [aberto]);
  // Fechar a tela no meio da digitação: o que estiver na fila termina de gravar.
  useEffect(() => () => { void gravacoesPendentes(); }, []);
  // Caderno que sumiu (lixeira em outra janela, pasta apagada): volta para a estante.
  useEffect(() => { if (aberto && lista && !caderno(aberto)) setAberto(null); }, [aberto, lista]);

  if (!lista) {
    const erro = erroDosCadernos();
    return erro ? <div className="p-6"><p role="alert" className="text-xs text-danger">Não consegui ler os cadernos: {erro}</p></div>
      : <div className="p-6 font-mono text-xs text-text-muted">Abrindo cadernos…</div>;
  }
  if (aberto && caderno(aberto)) return <CadernoAberto id={aberto} onVoltar={() => { setAberto(null); void carregarCadernos(); }} />;
  return <Estante lista={lista} onAbrir={setAberto} />;
}
