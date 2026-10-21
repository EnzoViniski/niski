// Fork (Editor de vídeo): propriedades do clipe selecionado ou, sem seleção, da composição.
import { useEffect, useState } from "react";
import { Button } from "../../components/ui";
import { duracaoTotal, duracaoVisivel, seg, type Clipe, type Composicao } from "./composicao";
import { formatarTempo } from "./relogio";

interface Props {
  comp: Composicao;
  clipe: Clipe | null;
  duracoes: Record<string, number>;
  bloqueado: boolean;
  onMudar: (chave: string, mudancas: Record<string, string | null>, motivo: string, novoFim?: number) => void;
  onDuracaoTotal: (s: number) => void;
  onDividir: () => void;
  onDuplicar: () => void;
  onApagar: () => void;
  onOcultar: () => void;
}

/** Campo numérico que só grava ao sair ou com Enter (digitar "1." não vira edição). */
function CampoDeNumero({ rotulo, valor, onGravar, min = 0, passo = 0.1, sufixo = "s", desligado }: {
  rotulo: string; valor: number | undefined; onGravar: (v: number) => void; min?: number; passo?: number; sufixo?: string; desligado?: boolean;
}) {
  const [texto, setTexto] = useState(valor === undefined ? "" : seg(valor));
  useEffect(() => { setTexto(valor === undefined ? "" : seg(valor)); }, [valor]);
  const gravar = () => {
    const v = Number(texto.replace(",", "."));
    if (!Number.isFinite(v) || v < min) { setTexto(valor === undefined ? "" : seg(valor)); return; }
    if (valor === undefined || Math.abs(v - valor) > 1e-4) onGravar(v);
  };
  return (
    <label className="video-prop-campo">
      <span>{rotulo}</span>
      <span className="video-prop-entrada">
        <input className="field px-2 py-1 text-xs" inputMode="decimal" aria-label={rotulo} value={texto} disabled={desligado} placeholder="auto"
          onChange={(e) => setTexto(e.target.value)} onBlur={gravar} step={passo}
          onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); gravar(); } e.stopPropagation(); }} />
        {sufixo && <small>{sufixo}</small>}
      </span>
    </label>
  );
}

export function Propriedades({ comp, clipe, duracoes, bloqueado, onMudar, onDuracaoTotal, onDividir, onDuplicar, onApagar, onOcultar }: Props) {
  if (!clipe) {
    const total = duracaoTotal(comp, duracoes);
    return (
      <div className="video-prop">
        <h3 className="video-prop-titulo">Vídeo</h3>
        {comp.raiz ? (
          <>
            <CampoDeNumero rotulo="Duração total" valor={comp.raiz.duracao ?? total} onGravar={onDuracaoTotal} min={0.5} desligado={bloqueado} />
            <p className="video-prop-info">{comp.raiz.largura}×{comp.raiz.altura} · {comp.raiz.fps} quadros por segundo · {comp.clipes.length} clipe{comp.clipes.length === 1 ? "" : "s"}</p>
          </>
        ) : <p className="video-prop-info">A composição não tem root. Peça ao agente para arrumar.</p>}
        <p className="video-prop-dica">Clique num clipe da timeline para ver e mudar início, duração, corte e volume. Espaço toca e pausa; ← → andam um quadro; S divide na agulha; ⌫ apaga; ⌘Z desfaz.</p>
      </div>
    );
  }
  const dur = duracaoVisivel(clipe, duracoes);
  const desligado = bloqueado || !clipe.editavel;
  const midia = clipe.tipo === "video" || clipe.tipo === "audio";
  return (
    <div className="video-prop">
      <h3 className="video-prop-titulo" title={clipe.id ? `id: ${clipe.id}` : undefined}>{clipe.rotulo}</h3>
      {!clipe.editavel && <p className="video-prop-aviso">O início deste clipe depende de outro. Peça ao agente para mudar.</p>}
      <div className="video-prop-grade">
        <CampoDeNumero rotulo="Início" valor={clipe.inicio} desligado={desligado}
          onGravar={(v) => onMudar(clipe.chave, { "data-start": seg(v) }, `Início de “${clipe.rotulo}” em ${formatarTempo(v)}`, v + dur)} />
        <CampoDeNumero rotulo="Duração" valor={clipe.duracao} desligado={bloqueado} min={0.05}
          onGravar={(v) => onMudar(clipe.chave, { "data-duration": seg(v) }, `Duração de “${clipe.rotulo}”: ${seg(v)} s`, clipe.inicio + v)} />
        <CampoDeNumero rotulo="Faixa" valor={clipe.faixa + 1} passo={1} min={1} sufixo="" desligado={bloqueado}
          onGravar={(v) => onMudar(clipe.chave, { "data-track-index": String(Math.max(0, Math.round(v) - 1)) }, `“${clipe.rotulo}” para a faixa ${Math.round(v)}`)} />
        {midia && <CampoDeNumero rotulo="Começa a mídia em" valor={clipe.midiaInicio} desligado={bloqueado}
          onGravar={(v) => onMudar(clipe.chave, { "data-media-start": seg(v) }, `Corte da mídia de “${clipe.rotulo}”`)} />}
      </div>
      {midia && (
        <label className="video-prop-campo is-largo">
          <span>Volume {Math.round((clipe.volume ?? 1) * 100)}%</span>
          <input type="range" min={0} max={2} step={0.05} defaultValue={clipe.volume ?? 1} disabled={bloqueado || (clipe.tipo === "video" && clipe.mudo)}
            key={`${clipe.chave}-${clipe.volume}`}
            onPointerUp={(e) => onMudar(clipe.chave, { "data-volume": seg(Number((e.target as HTMLInputElement).value)) }, `Volume de “${clipe.rotulo}”`)}
            onKeyUp={(e) => onMudar(clipe.chave, { "data-volume": seg(Number((e.target as HTMLInputElement).value)) }, `Volume de “${clipe.rotulo}”`)} />
        </label>
      )}
      <div className="video-prop-botoes">
        {clipe.tipo === "video" && (
          <Button size="sm" variant="ghost" disabled={bloqueado} aria-pressed={clipe.mudo}
            onClick={() => onMudar(clipe.chave, clipe.mudo ? { muted: null, "data-has-audio": "true" } : { muted: "", "data-has-audio": null }, clipe.mudo ? `Som de “${clipe.rotulo}” ligado` : `“${clipe.rotulo}” sem som`)}>
            {clipe.mudo ? "Ligar o som" : "Tirar o som"}
          </Button>
        )}
        <Button size="sm" variant="ghost" disabled={bloqueado} aria-pressed={clipe.oculto} onClick={onOcultar}>{clipe.oculto ? "Mostrar" : "Esconder"}</Button>
        <Button size="sm" variant="ghost" disabled={bloqueado} onClick={onDividir} title="Divide na agulha (S)">Dividir</Button>
        <Button size="sm" variant="ghost" disabled={bloqueado} onClick={onDuplicar} title="⌘D">Duplicar</Button>
        <Button size="sm" variant="danger" disabled={bloqueado} onClick={onApagar} title="⌫">Apagar</Button>
      </div>
      <p className="video-prop-info">{formatarTempo(clipe.inicio)} → {formatarTempo(clipe.inicio + dur)}{clipe.src ? ` · ${clipe.src}` : ""}</p>
    </div>
  );
}
