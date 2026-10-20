// Fork (Onda 2.0): saiu do `MobileApp.tsx` sem mudança de lógica.
import { useState } from "react";
import { ArchiveIcon } from "../../components/ui/PixelIcon";
import type { Conversation, Projects } from "../api";
import { ConversationList } from "./ConversationList";

/** Conversas que você ocultou: recolhidas no fim da lista. Abrir uma mostra o botão de reabrir. */
export function Ocultas({ conversas, perfis, projetos, mostrarProjeto = false, abrir, ativa = null }: {
  conversas: Conversation[]; perfis: Projects["profiles"]; projetos?: Projects["projects"];
  mostrarProjeto?: boolean; abrir: (id: string) => void; ativa?: string | null;
}) {
  const [aberto, setAberto] = useState(false);
  if (conversas.length === 0) return null;
  return <div className="m-ocultas">
    <button type="button" className="m-ocultas-botao" aria-expanded={aberto} onClick={() => setAberto(valor => !valor)}>
      <ArchiveIcon aria-hidden className="h-3.5 w-3.5" />
      <span>Ocultas ({conversas.length})</span>
      <span className="m-ocultas-acao">{aberto ? "esconder" : "ver"}</span>
    </button>
    {aberto && <>
      <p className="m-dica">Abra uma conversa e toque em <ArchiveIcon aria-hidden className="inline h-3 w-3" /> para ela voltar à lista.</p>
      <ConversationList conversations={conversas} perfis={perfis} projetos={projetos} mostrarProjeto={mostrarProjeto}
        abrir={abrir} ativa={ativa} vazio="" />
    </>}
  </div>;
}
