// Fork (Editor de vídeo, edição a dois): entrar na sessão de outra pessoa com o convite.
// O projeto vira uma cópia local (o mesmo convite de novo reaproveita a cópia).
import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { Button } from "../../../components/ui";
import * as disco from "../armazenamento";
import { indexInicial, instrucoesDoAgente } from "../modelos";
import { PassoAPassoDoTailscale } from "./SessaoADois";

interface ConviteLido { v: number; bases: string[]; p: string; n: string }

export function EntrarNaSessao({ onEntrou, onFechar }: { onEntrou: (projetoLocal: string) => void; onFechar: () => void }) {
  const [texto, setTexto] = useState("");
  const [lido, setLido] = useState<ConviteLido | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [entrando, setEntrando] = useState(false);

  const ler = async (t: string) => {
    setTexto(t); setErro(null); setLido(null);
    if (!t.trim()) return;
    try { setLido(await invoke<ConviteLido>("videos_sessao_ler_convite", { convite: t.trim() })); } catch (e) { setErro(String(e)); }
  };

  const entrar = async () => {
    if (!lido) return;
    setEntrando(true); setErro(null);
    try {
      const lista = await disco.listarProjetos();
      let local = lista.find((i) => i.projeto.aDois?.projetoRemoto === lido.p && i.projeto.aDois?.host === lido.n)?.id;
      local ??= await disco.criarProjeto({
        projeto: { nome: `${lido.n} · a dois`, formato: "16:9", largura: 1920, altura: 1080, fps: 30, cor: "#22d3ee", ia: { cli: "claude" }, briefing: {}, pastasDeReferencia: [], sessoes: {}, aDois: { projetoRemoto: lido.p, host: lido.n } },
        brief: "# Esperando a sessão a dois…\n", index: indexInicial({ nome: "Esperando a sessão…", formato: "16:9", duracao: 5 }), instrucoes: instrucoesDoAgente(),
      });
      await invoke("videos_sessao_entrar", { convite: texto.trim(), projetoLocal: local });
      onEntrou(local);
    } catch (e) { setErro(String(e)); setEntrando(false); }
  };

  return (
    <div className="video-modal-fundo" role="presentation" onPointerDown={(e) => { if (e.target === e.currentTarget) onFechar(); }}>
      <div className="video-modal glass" role="dialog" aria-modal="true" aria-label="Entrar numa sessão a dois" onKeyDown={(e) => { e.stopPropagation(); if (e.key === "Escape") onFechar(); }}>
        <h2 className="video-modal-titulo">Entrar numa sessão a dois</h2>
        <label className="video-campo"><span>Cole o convite</span>
          <textarea className="field px-2.5 py-1.5 text-xs" rows={3} value={texto} placeholder="medza-video:…" onChange={(e) => void ler(e.target.value)} />
        </label>
        {lido && <p className="video-ok">Sessão de {lido.n}. Você vai editar uma cópia neste PC, sincronizada com a dele.</p>}
        {erro && <p className="video-erro" role="alert">{erro}</p>}
        <PassoAPassoDoTailscale />
        <div className="video-modal-botoes">
          <Button size="sm" variant="ghost" onClick={onFechar}>Cancelar</Button>
          <Button size="sm" variant="primary" disabled={!lido || entrando} loading={entrando} onClick={() => void entrar()}>Entrar</Button>
        </div>
      </div>
    </div>
  );
}
