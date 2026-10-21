// Fork (Editor de vídeo, edição a dois): abrir/encerrar a sessão, o convite, quem pode entrar e o
// passo a passo do Tailscale.
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Button } from "../../../components/ui";
import { CheckIcon, CopyIcon, WarningIcon } from "../../../components/ui/PixelIcon";
import type { SessaoADois, SituacaoDaSessao } from "./useSessaoADois";

export function PassoAPassoDoTailscale() {
  return (
    <details className="video-requisitos">
      <summary>Como configurar o Tailscale (uma vez)</summary>
      <ol className="video-passos">
        <li>Os dois instalam o Tailscale e entram cada um na sua conta.</li>
        <li>Quem vai abrir a sessão compartilha <b>este PC</b> com o outro: no painel do Tailscale, Machines → este PC → Share → mande o convite. É de graça e o outro só enxerga esta máquina.</li>
        <li>Abra a sessão aqui e mande o convite (o texto que começa com <code>medza-video:</code>) pelo WhatsApp.</li>
        <li>Na primeira vez, o login do Tailscale do outro aparece aqui como pedido: clique em Permitir.</li>
      </ol>
      <p className="video-dica">A sessão fica só dentro do Tailscale (<code>tailscale serve</code>, nunca Funnel): ninguém de fora da tailnet alcança. Cada um usa o próprio agente e a própria conta; enquanto um está com o agente, o outro espera.</p>
    </details>
  );
}

export function SessaoADoisDialogo({ sessao, onFechar }: { sessao: SessaoADois; onFechar: () => void }) {
  const [situacao, setSituacao] = useState<SituacaoDaSessao | null>(null);
  const [nome, setNome] = useState("");
  const [permitidos, setPermitidos] = useState("");
  const [copiado, setCopiado] = useState(false);
  const [abrindo, setAbrindo] = useState(false);

  const ler = () => void invoke<SituacaoDaSessao>("videos_sessao_situacao").then((s) => {
    setSituacao(s);
    setNome(s.config?.nome ?? "");
    setPermitidos((s.config?.permitidos ?? []).join("\n"));
  }).catch(() => undefined);
  useEffect(ler, []);

  const gravar = async () => {
    await invoke("videos_sessao_config_gravar", { nome: nome.trim(), permitidos: permitidos.split(/[\s,;]+/).filter(Boolean) }).catch(() => undefined);
  };

  const ts = situacao?.tailscale;
  return (
    <div className="video-modal-fundo" role="presentation" onPointerDown={(e) => { if (e.target === e.currentTarget) onFechar(); }}>
      <div className="video-modal glass" role="dialog" aria-modal="true" aria-label="Editar a dois" onKeyDown={(e) => { e.stopPropagation(); if (e.key === "Escape") onFechar(); }}>
        <h2 className="video-modal-titulo">Editar a dois</h2>
        {ts && (ts.pronto
          ? <p className="video-ok"><CheckIcon size={12} aria-hidden="true" /> Tailscale ligado{ts.login ? ` como ${ts.login}` : ""}{ts.dns ? ` · ${ts.dns}` : ""}</p>
          : <p className="video-aviso"><WarningIcon size={12} aria-hidden="true" /> {ts.erro ?? "Tailscale indisponível."}</p>)}

        {!sessao.papel && (
          <>
            <label className="video-campo"><span>Seu nome na sessão</span>
              <input className="field px-2.5 py-1.5 text-xs" value={nome} placeholder={ts?.login ?? "Ex.: Enzo"} onChange={(e) => setNome(e.target.value)} onBlur={() => void gravar()} />
            </label>
            <label className="video-campo"><span>Quem pode entrar (login do Tailscale, um por linha)</span>
              <textarea className="field px-2.5 py-1.5 text-xs" rows={3} value={permitidos} placeholder="primo@gmail.com" onChange={(e) => setPermitidos(e.target.value)} onBlur={() => void gravar()} />
            </label>
            <PassoAPassoDoTailscale />
            <p className="video-dica">Antes de abrir, o app guarda uma cópia do projeto em ~/omni-backups/videos/.</p>
          </>
        )}

        {sessao.papel === "host" && (
          <>
            <p className="video-dica">Sessão aberta. Mande este convite para quem vai editar junto:</p>
            {sessao.convite && (
              <div className="video-convite">
                <code>{sessao.convite}</code>
                <Button size="sm" variant="glass" onClick={() => void navigator.clipboard?.writeText(sessao.convite!).then(() => { setCopiado(true); window.setTimeout(() => setCopiado(false), 1500); })}>
                  <CopyIcon size={12} /> {copiado ? "Copiado" : "Copiar"}
                </Button>
              </div>
            )}
            {sessao.pedidos.map((login) => (
              <p key={login} className="video-aviso" role="alert">{login} quer entrar.
                <Button size="sm" variant="primary" onClick={() => void sessao.permitir(login)}>Permitir</Button>
              </p>
            ))}
            <p className="video-dica">{sessao.colegas.length ? `Na sessão: ${sessao.colegas.map((c) => c.nome).join(", ")}` : "Ninguém entrou ainda."}</p>
          </>
        )}
        {sessao.papel === "convidado" && (
          <p className="video-dica">{sessao.conexao === "ok" ? "Você está numa sessão a dois." : "A conexão caiu; o app tenta de novo sozinho."} {sessao.colegas.length ? `Com: ${sessao.colegas.map((c) => c.nome).join(", ")}` : ""}</p>
        )}
        {sessao.erro && <p className="video-erro" role="alert">{sessao.erro}</p>}

        <div className="video-modal-botoes">
          <Button size="sm" variant="ghost" onClick={onFechar}>Fechar</Button>
          {!sessao.papel && (
            <Button size="sm" variant="primary" loading={abrindo} disabled={!ts?.pronto || abrindo}
              onClick={() => { setAbrindo(true); void gravar().then(() => sessao.iniciar()).finally(() => { setAbrindo(false); ler(); }); }}>Abrir sessão a dois</Button>
          )}
          {sessao.papel && <Button size="sm" variant="danger" onClick={() => void sessao.parar().then(ler)}>{sessao.papel === "host" ? "Encerrar a sessão" : "Sair da sessão"}</Button>}
        </div>
      </div>
    </div>
  );
}
