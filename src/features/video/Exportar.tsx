// Fork (Editor de vídeo): exportar o vídeo no PC (render local do HyperFrames, nada vai para a nuvem).
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Button, SegmentedControl } from "../../components/ui";
import { CheckIcon, LoaderIcon, WarningIcon } from "../../components/ui/PixelIcon";

type Formato = "mp4" | "mov" | "webm" | "gif";
type Qualidade = "draft" | "standard" | "high";
interface Checagem { name: string; ok: boolean; detail: string; hint?: string }
interface Requisitos { ok: boolean; checks?: Checagem[] }

/** O que o render precisa de verdade (o doctor também lista extras opcionais, como TTS e Docker). */
const ESSENCIAIS = /node|ffmpeg|ffprobe|chrome|browser|disk/i;

export function Exportar({ projeto, nome, onFechar }: { projeto: string; nome: string; onFechar: () => void }) {
  const [formato, setFormato] = useState<Formato>("mp4");
  const [qualidade, setQualidade] = useState<Qualidade>("standard");
  const [fps, setFps] = useState<"" | "30" | "60">("");
  const [estado, setEstado] = useState<"parado" | "exportando" | "pronto" | "erro">("parado");
  const [progresso, setProgresso] = useState<{ pct: number; etapa: string }>({ pct: 0, etapa: "" });
  const [mensagem, setMensagem] = useState("");
  const [arquivo, setArquivo] = useState<string | null>(null);
  const [requisitos, setRequisitos] = useState<Requisitos | null>(null);
  const [conferindo, setConferindo] = useState(false);
  const [preparando, setPreparando] = useState(false);

  useEffect(() => {
    if (estado !== "exportando") return;
    let parar: (() => void) | undefined;
    let vivo = true;
    void import("@tauri-apps/api/event").then(({ listen }) => listen<{ projeto: string; pct: number; etapa: string }>("video-render", ({ payload }) => {
      if (payload.projeto === projeto) setProgresso({ pct: payload.pct, etapa: payload.etapa });
    })).then((p) => { if (vivo) parar = p; else p(); }).catch(() => undefined);
    return () => { vivo = false; parar?.(); };
  }, [estado, projeto]);

  const exportar = async () => {
    setEstado("exportando"); setProgresso({ pct: 0, etapa: "Preparando (a primeira vez baixa o HyperFrames)…" }); setMensagem("");
    try {
      const caminho = await invoke<string>("videos_exportar", { pedido: { projeto, formato, qualidade, fps: fps ? Number(fps) : null, nome } });
      setArquivo(caminho); setEstado("pronto");
    } catch (e) {
      const t = e instanceof Error ? e.message : String(e);
      setMensagem(t === "Cancelado." ? "Exportação cancelada." : t); setEstado(t === "Cancelado." ? "parado" : "erro");
    }
  };

  const conferir = async () => {
    setConferindo(true);
    try { setRequisitos(await invoke<Requisitos>("videos_requisitos")); }
    catch (e) { setRequisitos({ ok: false, checks: [{ name: "Node.js", ok: false, detail: e instanceof Error ? e.message : String(e) }] }); }
    finally { setConferindo(false); }
  };

  const essenciais = (requisitos?.checks ?? []).filter((c) => ESSENCIAIS.test(c.name));
  const faltaNavegador = essenciais.some((c) => /chrome|browser/i.test(c.name) && !c.ok);

  return (
    <div className="video-modal-fundo" role="presentation" onPointerDown={(e) => { if (e.target === e.currentTarget && estado !== "exportando") onFechar(); }}>
      <div className="video-modal glass" role="dialog" aria-modal="true" aria-label="Exportar vídeo" onKeyDown={(e) => { e.stopPropagation(); if (e.key === "Escape" && estado !== "exportando") onFechar(); }}>
        <h2 className="video-modal-titulo">Exportar vídeo</h2>
        <p className="video-dica">Renderiza aqui no PC, quadro a quadro, com prioridade baixa. Nada sobe para a nuvem.</p>
        <SegmentedControl<Formato> label="Formato" value={formato} onChange={setFormato}
          options={[{ value: "mp4", label: "MP4" }, { value: "mov", label: "MOV", title: "Mantém transparência" }, { value: "webm", label: "WebM", title: "Mantém transparência" }, { value: "gif", label: "GIF" }]} />
        <SegmentedControl<Qualidade> label="Qualidade" value={qualidade} onChange={setQualidade}
          options={[{ value: "draft", label: "Rascunho", title: "Rápido, para conferir" }, { value: "standard", label: "Padrão" }, { value: "high", label: "Alta", title: "Para publicar; demora mais" }]} />
        <SegmentedControl<"" | "30" | "60"> label="Quadros por segundo" value={fps} onChange={setFps}
          options={[{ value: "", label: "Do projeto" }, { value: "30", label: "30" }, { value: "60", label: "60" }]} />

        {estado === "exportando" && (
          <div className="video-progresso" role="status" aria-live="polite">
            <div className="video-barra"><span style={{ width: `${progresso.pct}%` }} /></div>
            <p><LoaderIcon size={12} className="animate-spin" aria-hidden="true" /> {Math.round(progresso.pct)}% · {progresso.etapa}</p>
          </div>
        )}
        {estado === "pronto" && arquivo && (
          <div className="video-ok" role="status"><CheckIcon size={14} aria-hidden="true" /> Pronto: {arquivo.split(/[\\/]/).pop()}
            <Button size="sm" variant="glass" onClick={() => void import("@tauri-apps/plugin-opener").then((o) => o.revealItemInDir(arquivo))}>Mostrar no Finder</Button>
            <Button size="sm" variant="ghost" onClick={() => void import("@tauri-apps/plugin-opener").then((o) => o.openPath(arquivo))}>Abrir</Button>
          </div>
        )}
        {mensagem && <p className={estado === "erro" ? "video-erro" : "video-aviso"} role="alert">{mensagem}</p>}

        <details className="video-requisitos" onToggle={(e) => { if ((e.target as HTMLDetailsElement).open && !requisitos && !conferindo) void conferir(); }} open={estado === "erro" || undefined}>
          <summary>Requisitos (Node 22+, FFmpeg e o Chrome do render)</summary>
          {conferindo && <p className="video-dica"><LoaderIcon size={12} className="animate-spin" aria-hidden="true" /> Conferindo…</p>}
          {essenciais.map((c) => (
            <p key={c.name} className={`video-requisito${c.ok ? "" : " is-falta"}`}>{c.ok ? <CheckIcon size={12} aria-hidden="true" /> : <WarningIcon size={12} aria-hidden="true" />} <b>{c.name}</b> {c.detail}{!c.ok && c.hint ? ` · ${c.hint}` : ""}</p>
          ))}
          {faltaNavegador && (
            <Button size="sm" variant="glass" loading={preparando} onClick={() => { setPreparando(true); void invoke("videos_preparar_navegador").then(conferir).catch((e) => setMensagem(String(e))).finally(() => setPreparando(false)); }}>
              Baixar o Chrome do render (uma vez)
            </Button>
          )}
          {requisitos && <Button size="sm" variant="ghost" onClick={() => void conferir()}>Conferir de novo</Button>}
        </details>

        <div className="video-modal-botoes">
          {estado === "exportando"
            ? <Button size="sm" variant="danger" onClick={() => void invoke("videos_exportar_cancelar", { projeto })}>Cancelar</Button>
            : <Button size="sm" variant="ghost" onClick={onFechar}>Fechar</Button>}
          <Button size="sm" variant="primary" disabled={estado === "exportando"} onClick={() => void exportar()}>{estado === "pronto" ? "Exportar de novo" : "Exportar"}</Button>
        </div>
      </div>
    </div>
  );
}
