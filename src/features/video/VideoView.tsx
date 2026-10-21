// Fork (Editor de vídeo): a tela — lista de vídeos e pastas, criar, editar pasta ou o editor.
import { useEffect, useState } from "react";
import { Editor } from "./Editor";
import { ListaDeVideos } from "./ListaDeVideos";
import { NovoVideo } from "./NovoVideo";
import { PastaDeReferencias } from "./PastaDeReferencias";
import "./video.css";
import type { AgentView } from "../../types/settings";

type Tela = { tipo: "lista" } | { tipo: "novo"; pasta?: string; computador?: string } | { tipo: "editor"; id: string } | { tipo: "pasta"; id: string };

const ULTIMO = "medza.video.ultimo";

function lembrar(id: string | null) {
  try { if (id) localStorage.setItem(ULTIMO, id); else localStorage.removeItem(ULTIMO); } catch { /* sem armazenamento: só não lembra */ }
}
function ultimo() { try { return localStorage.getItem(ULTIMO); } catch { return null; } }

export function VideoView({ onClose, continuar = false, agentView = "conversa" }: { onClose: () => void; continuar?: boolean; agentView?: AgentView }) {
  const [tela, setTela] = useState<Tela>(() => { const u = continuar ? ultimo() : null; return u ? { tipo: "editor", id: u } : { tipo: "lista" }; });
  const [aviso, setAviso] = useState<string | null>(null);
  useEffect(() => { if (tela.tipo === "editor") lembrar(tela.id); }, [tela]);

  const voltar = () => { lembrar(null); setTela({ tipo: "lista" }); };
  return (
    <div className="video-tela">
      {aviso && <p className="video-aviso video-aviso-topo" role="alert">{aviso}<button type="button" className="video-mini-botao" aria-label="Fechar aviso" onClick={() => setAviso(null)}>×</button></p>}
      {tela.tipo === "lista" && <ListaDeVideos onAbrir={(id) => setTela({ tipo: "editor", id })} onNovo={(computador) => setTela({ tipo: "novo", ...(computador ? { computador } : {}) })} onAbrirPasta={(id) => setTela({ tipo: "pasta", id })} onFechar={onClose} />}
      {tela.tipo === "novo" && <NovoVideo pastaInicial={tela.pasta} computadorInicial={tela.computador} onCancelar={() => setTela({ tipo: "lista" })} onCriado={(id, a) => { setAviso(a ?? null); setTela({ tipo: "editor", id }); }} />}
      {tela.tipo === "pasta" && <PastaDeReferencias id={tela.id} onVoltar={() => setTela({ tipo: "lista" })} onNovoVideo={(pasta) => setTela({ tipo: "novo", pasta })} />}
      {tela.tipo === "editor" && <Editor key={tela.id} id={tela.id} onVoltar={voltar} agentView={agentView} />}
    </div>
  );
}
