// Fork (Tarefas): o botão com a logo no canto inferior direito (abre o painel; arrastável) e a lista
// do dia flutuante (fixada pelo 📌, arrastável pelo topo, recolhível).
import { useEffect, useRef, useState, type PointerEvent as PointerReact } from "react";
import { MarcaDoProduto } from "../../components/ui/MarcaDoProduto";
import { ClipboardNoteIcon } from "../../components/ui/PixelIcon";
import { alternarCopiloto, useCopiloto } from "../copiloto/copiloto";
import { BotaoDeConcluir } from "./CartaoDaTarefa";
import { aberta, concluir, diaDe, lerCriacaoRapida, montarDia, reabrir } from "./dominio";
import { adicionar, pessoasConhecidas, salvar, useTarefas } from "./loja";
import { abrirPainelDeTarefas, alternarListaDoDia, alternarPainelDeTarefas, dentroDaJanela, mudarPainel, usePainel, type Posicao } from "./painel";
import { categoria } from "./tipos";
// Fork (revisão do copiloto): o lançador desvia do campo de mensagem e da conversa do editor de vídeo.
import { desviarLancador, OBSTACULOS_DO_LANCADOR, type Recuo } from "./desvioDoLancador";
import "./painel.css";
import { energiaPausada } from "../../lib/energia";

/**
 * Arrastar com o ponteiro: devolve o handler do `onPointerDown` e avisa se houve movimento (para o
 * clique do fim do arrasto não contar como clique).
 */
function useArrastar(aoMover: (dx: number, dy: number) => void, aoSoltar: () => void) {
  const inicio = useRef<{ x: number; y: number; moveu: boolean } | null>(null);
  const moveu = useRef(false);
  const callbacks = useRef({ aoMover, aoSoltar });
  callbacks.current = { aoMover, aoSoltar };
  useEffect(() => {
    const mover = (e: PointerEvent) => {
      const s = inicio.current;
      if (!s) return;
      const dx = e.clientX - s.x, dy = e.clientY - s.y;
      if (!s.moveu && Math.hypot(dx, dy) < 4) return;
      s.moveu = true;
      callbacks.current.aoMover(dx, dy);
    };
    const subir = () => {
      if (!inicio.current) return;
      moveu.current = inicio.current.moveu;
      if (inicio.current.moveu) callbacks.current.aoSoltar();
      inicio.current = null;
      setTimeout(() => { moveu.current = false; }, 0);
    };
    window.addEventListener("pointermove", mover);
    window.addEventListener("pointerup", subir);
    return () => { window.removeEventListener("pointermove", mover); window.removeEventListener("pointerup", subir); };
  }, []);
  const iniciar = (e: PointerReact<HTMLElement>) => {
    if (e.button !== 0 || (e.target as HTMLElement).closest("button:not([data-alca]), input, textarea")) return;
    inicio.current = { x: e.clientX, y: e.clientY, moveu: false };
  };
  return { iniciar, moveu };
}

function useContagemDeHoje() {
  const e = useTarefas();
  const hoje = diaDe(new Date());
  const eu = e.ajustes.eu.id;
  return e.tarefas.filter((t) => aberta(t) && t.data && t.data <= hoje && (!eu || !t.responsavel || t.responsavel === eu)).length;
}

const LADO_DO_BOTAO = 48;

/**
 * Fork (Copiloto): os botões do canto inferior direito, arrastáveis juntos. Embaixo, a logo do Niski
 * abre o copiloto; em cima, o botão de Tarefas (com o número de hoje) abre o painel de tarefas; o 📌
 * aparece ao lado dele e fixa a lista do dia. Com o copiloto preso à direita, os botões andam para o
 * lado dele.
 */
export function BotaoDasTarefas() {
  const painel = usePainel();
  const copiloto = useCopiloto();
  const n = useContagemDeHoje();
  const [arrasto, setArrasto] = useState<Posicao | null>(null);
  const [desvio, setDesvio] = useState<Recuo | null>(null);
  const caixa = useRef<HTMLDivElement>(null);
  const base = useRef(painel.posBotao);
  const { iniciar, moveu } = useArrastar(
    (dx, dy) => {
      const p = dentroDaJanela({ x: window.innerWidth - base.current.x - LADO_DO_BOTAO + dx, y: window.innerHeight - base.current.y - LADO_DO_BOTAO + dy }, LADO_DO_BOTAO, LADO_DO_BOTAO * 2);
      setArrasto({ x: window.innerWidth - p.x - LADO_DO_BOTAO, y: window.innerHeight - p.y - LADO_DO_BOTAO });
    },
    () => setArrasto((p) => { if (p) mudarPainel({ posBotao: p }); return null; }),
  );
  const ladoDoCopiloto = copiloto.aberto && copiloto.modo === "direita" ? copiloto.larguraDireita : 0;
  const visivel = painel.botaoVisivel && !painel.aberto;
  // Confere de tempos em tempos (telas trocam, o composer cresce, a janela muda de tamanho). Barato:
  // um punhado de caixas; parado com a janela escondida e durante o arrasto.
  useEffect(() => {
    if (!visivel || arrasto) return;
    const conferir = () => {
      const el = caixa.current;
      if (!el || document.hidden) return;
      const { width, height } = el.getBoundingClientRect();
      const obstaculos = [...document.querySelectorAll(OBSTACULOS_DO_LANCADOR)]
        .filter((o) => !el.contains(o)).map((o) => o.getBoundingClientRect());
      const base = { right: painel.posBotao.x + ladoDoCopiloto, bottom: painel.posBotao.y };
      const novo = desviarLancador(base, width || LADO_DO_BOTAO, height || LADO_DO_BOTAO * 2, obstaculos, { w: window.innerWidth, h: window.innerHeight });
      setDesvio((atual) => (atual?.right === novo.right && atual?.bottom === novo.bottom ? atual : novo));
    };
    conferir();
    // Fork (desempenho): na pausa de energia (sem foco ou parado) não força layout a cada 700 ms.
    const relogio = window.setInterval(() => { if (!energiaPausada()) conferir(); }, 700);
    window.addEventListener("resize", conferir);
    return () => { window.clearInterval(relogio); window.removeEventListener("resize", conferir); };
  }, [visivel, arrasto, painel.posBotao.x, painel.posBotao.y, ladoDoCopiloto]);
  if (!visivel) return null;
  const recuo = arrasto ? { right: arrasto.x + ladoDoCopiloto, bottom: arrasto.y } : desvio ?? { right: painel.posBotao.x + ladoDoCopiloto, bottom: painel.posBotao.y };
  // O arrasto parte de onde o lançador está na tela (já com o desvio), não da posição salva.
  const pegar = (e: PointerReact<HTMLElement>) => { base.current = { x: recuo.right - ladoDoCopiloto, y: recuo.bottom }; iniciar(e); };
  return (
    <div ref={caixa} className="tarefas-lancador" style={{ right: recuo.right, bottom: recuo.bottom }}>
      <div className="tarefas-lancador-linha">
        <button type="button" className="tarefas-lancador-lista" title={painel.listaFixada ? "Esconder a lista do dia" : "Lista do dia flutuante"}
          aria-pressed={painel.listaFixada} onClick={alternarListaDoDia}>📌</button>
        <button type="button" data-alca className="tarefas-lancador-tarefas"
          aria-label={`Tarefas${n ? ` (${n} para hoje)` : ""}`} title="Tarefas: meu dia, quadro e assistente"
          onPointerDown={pegar} onClick={() => { if (!moveu.current) alternarPainelDeTarefas(); }}>
          <ClipboardNoteIcon className="h-[18px] w-[18px]" aria-hidden />
          {n > 0 && <span className="tarefas-lancador-selo">{n > 99 ? "99+" : n}</span>}
        </button>
      </div>
      <button type="button" data-alca className={`tarefas-lancador-botao${copiloto.aberto ? " is-aberto" : ""}`}
        aria-label="Copiloto" title="Copiloto do Niski: converse e ele faz no app e no PC (arraste para mudar de lugar)"
        onPointerDown={pegar} onClick={() => { if (!moveu.current) alternarCopiloto(); }}>
        <MarcaDoProduto size={26} aria-hidden />
      </button>
    </div>
  );
}

const LARGURA_DA_LISTA = 300;

export function ListaDoDia() {
  const painel = usePainel();
  const e = useTarefas();
  const [texto, setTexto] = useState("");
  const [arrasto, setArrasto] = useState<Posicao | null>(null);
  const caixa = useRef<HTMLDivElement>(null);
  const base = useRef<Posicao>({ x: 0, y: 0 });
  const { iniciar } = useArrastar(
    (dx, dy) => setArrasto(dentroDaJanela({ x: base.current.x + dx, y: base.current.y + dy }, LARGURA_DA_LISTA, caixa.current?.offsetHeight ?? 200)),
    () => setArrasto((p) => { if (p) mudarPainel({ posLista: p }); return null; }),
  );
  if (!painel.listaFixada || painel.aberto) return null;

  const hoje = diaDe(new Date());
  const eu = e.ajustes.eu.id;
  const minhas = e.tarefas.filter((t) => !eu || !t.responsavel || t.responsavel === eu);
  const dia = montarDia(minhas, hoje);
  const lista = [...dia.atrasadas, ...dia.hoje];
  const feitas = dia.feitasHoje.length;
  const total = lista.length + feitas;
  const salva = painel.posLista ? dentroDaJanela(painel.posLista, LARGURA_DA_LISTA, 120) : null;
  const pos = arrasto ?? salva;
  const estilo = pos ? { left: pos.x, top: pos.y } : { right: 20, top: 72 };

  const criar = () => {
    const r = lerCriacaoRapida(texto, hoje, pessoasConhecidas(e), eu);
    if (!r.titulo) return;
    adicionar({ ...r, data: r.data || hoje });
    setTexto("");
  };

  return (
    <div ref={caixa} className="lista-do-dia" style={estilo} role="complementary" aria-label="Lista do dia">
      <header className="lista-do-dia-topo" onPointerDown={(ev) => {
        const r = caixa.current?.getBoundingClientRect();
        base.current = { x: r?.left ?? 0, y: r?.top ?? 0 };
        iniciar(ev);
      }}>
        <span className="lista-do-dia-pega" aria-hidden>⋮⋮</span>
        <strong className="flex-1">Hoje</strong>
        <span className="text-xs text-text-muted">{feitas}/{total}</span>
        <button type="button" className="tarefa-mini" title="Abrir o painel de tarefas" onClick={abrirPainelDeTarefas}>↗</button>
        <button type="button" className="tarefa-mini" aria-label={painel.listaRecolhida ? "Mostrar a lista" : "Recolher a lista"} onClick={() => mudarPainel({ listaRecolhida: !painel.listaRecolhida })}>{painel.listaRecolhida ? "▸" : "▾"}</button>
        <button type="button" className="tarefa-mini" aria-label="Fechar a lista do dia" onClick={alternarListaDoDia}>×</button>
      </header>
      {!painel.listaRecolhida && (
        <>
          <div className="lista-do-dia-barra"><span style={{ width: `${total ? (feitas / total) * 100 : 0}%` }} /></div>
          <ul className="lista-do-dia-itens">
            {lista.map((t) => (
              <li key={t.id} className={t.data < hoje ? "is-atrasada" : ""}>
                <BotaoDeConcluir feita={false} titulo={t.titulo} onClick={() => void salvar(concluir(t)).catch(() => undefined)} />
                <span className="min-w-0 flex-1">
                  <span className="block truncate">{t.titulo}</span>
                  <small>{t.data < hoje ? "Atrasada" : t.hora || "Hoje"} · {categoria(t.categoria).emoji}</small>
                </span>
              </li>
            ))}
            {dia.feitasHoje.slice(0, 3).map((t) => (
              <li key={t.id} className="is-feita">
                <BotaoDeConcluir feita titulo={t.titulo} onClick={() => void salvar(reabrir(t)).catch(() => undefined)} />
                <span className="min-w-0 flex-1 truncate">{t.titulo}</span>
              </li>
            ))}
            {!total && <li className="text-xs text-text-muted">Nada para hoje. 🎉</li>}
          </ul>
          <input className="lista-do-dia-campo" value={texto} placeholder="+ Para hoje… (Enter)" aria-label="Nova tarefa para hoje"
            onChange={(ev) => setTexto(ev.target.value)} onKeyDown={(ev) => { if (ev.key === "Enter") criar(); }} />
        </>
      )}
    </div>
  );
}
