import { useEffect, useState } from "react";
import { request, type Conversation } from "../../mobile/api";
import { ConversationDetail } from "../../mobile/MobileApp";

/** Histórico sem PTY usa o mesmo chat e a mesma retomada da web. */
export function ConversaDoHistorico({ id, aoRetomar }: { id: string; aoRetomar: (id: string) => void }) {
  const [conversa, setConversa] = useState<Conversation | null>(null);
  const [erro, setErro] = useState(false);
  useEffect(() => {
    let cancelado = false;
    setConversa(null); setErro(false);
    void request<Conversation[]>("/conversas").then((lista) => {
      if (!cancelado) { setConversa(lista.find((item) => item.id === id) ?? null); setErro(!lista.some((item) => item.id === id)); }
    }).catch(() => { if (!cancelado) setErro(true); });
    return () => { cancelado = true; };
  }, [id]);
  if (erro) return <p role="alert" className="p-4 text-sm text-danger">Não foi possível abrir esta conversa.</p>;
  if (!conversa) return <p role="status" className="p-4 text-sm text-text-muted">Abrindo conversa…</p>;
  return <ConversationDetail key={id} conversation={conversa} refresh={async () => undefined} abrirConversa={aoRetomar} />;
}
