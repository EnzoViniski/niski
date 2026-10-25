// Fork (Bíblia do dia): a leitura em tela cheia — uma passagem por trilha, com a referência, o texto
// exato da Bíblia Ave Maria e três saídas: "Li", "Deixar para amanhã" e "Ler mais tarde".
import { useEffect, useState } from "react";
import { Button } from "../../components/ui";
import { diaLocal } from "../manha/rotina";
import { abrirBiblia, useBiblia } from "./arquivo";
import { referencia, SOBRE_A_TRILHA, versiculosDa, type Biblia, type Passagem } from "./biblia";
import { decidir, diasSeguidos, leituraDeAgora, lidaHoje, lidasHoje, marcarComoLida, useLeitura, type LeituraDoDia as Leitura } from "./leitura";
import "./biblia.css";

export function LeituraDoDia({ onFechar, onAjustes }: { onFechar: () => void; onAjustes: () => void }) {
  const arquivo = useBiblia();
  const g = useLeitura();
  const agora = new Date();
  // Já leu hoje: mostra o que leu; "Ler mais" troca para as próximas.
  const [maisUma, setMaisUma] = useState(false);
  const [acabou, setAcabou] = useState(false);
  const jaLeu = lidaHoje(g, agora);

  const maisTarde = () => { decidir("depois"); onFechar(); };
  useEffect(() => {
    const tecla = (e: KeyboardEvent) => { if (e.key === "Escape") { e.preventDefault(); if (jaLeu || acabou) onFechar(); else maisTarde(); } };
    window.addEventListener("keydown", tecla);
    return () => window.removeEventListener("keydown", tecla);
  });

  const data = agora.toLocaleDateString("pt-BR", { weekday: "long", day: "numeric", month: "long" });

  return (
    <div className={`biblia${g.ajustes.letra === "grande" ? " is-letra-grande" : ""}`} role="dialog" aria-modal="true" aria-label="Palavra do dia">
      <div className="biblia-fundo" aria-hidden="true" />
      <header className="biblia-topo" data-tauri-drag-region>
        <div>
          <p className="biblia-sobre">{data}</p>
          <h1 className="biblia-titulo">Palavra do dia</h1>
        </div>
        <span className="flex-1" />
        {diasSeguidos(g, agora) > 0 && <span className="biblia-selo" title="Dias seguidos de leitura">🔥 {diasSeguidos(g, agora)}</span>}
        <Button size="sm" variant="ghost" onClick={jaLeu || acabou ? onFechar : maisTarde} aria-label="Fechar a leitura">Fechar</Button>
      </header>

      <main className="biblia-centro">
        {arquivo.fase === "erro" ? (
          <section className="biblia-folha biblia-aviso" role="alert">
            <p>Não consegui abrir a Bíblia: {arquivo.erro}</p>
            <div className="flex flex-wrap justify-center gap-2">
              <Button variant="ghost" onClick={maisTarde}>Ler mais tarde</Button>
              <Button variant="primary" onClick={() => void abrirBiblia()}>Tentar de novo</Button>
            </div>
          </section>
        ) : arquivo.fase !== "pronta" ? (
          <section className="biblia-folha biblia-aviso" role="status">
            <span className="biblia-pulso" aria-hidden="true" />
            {arquivo.fase === "baixando" ? "Baixando a Bíblia Ave Maria (só na primeira vez, 6,6 MB)…" : "Abrindo a Bíblia…"}
          </section>
        ) : !g.ajustes.trilhas.length ? (
          <section className="biblia-folha biblia-aviso">
            <p>Nenhuma trilha de leitura escolhida.</p>
            <Button variant="primary" onClick={() => { onAjustes(); onFechar(); }}>Escolher nas Configurações</Button>
          </section>
        ) : acabou ? (
          <Amem onFechar={onFechar} />
        ) : jaLeu && !maisUma ? (
          <JaLida biblia={arquivo.biblia} onMais={() => setMaisUma(true)} onFechar={onFechar} />
        ) : (
          <ParaLer biblia={arquivo.biblia} extra={jaLeu}
            onLi={(leitura) => { marcarComoLida(arquivo.biblia, leitura, leitura.map((l) => referencia(arquivo.biblia, l.passagem))); setMaisUma(false); setAcabou(true); }}
            onAmanha={() => { decidir("amanha"); onFechar(); }} onMaisTarde={maisTarde} />
        )}
      </main>
    </div>
  );
}

function Trecho({ biblia, passagem, rotulo, progresso }: { biblia: Biblia; passagem: Passagem; rotulo: string; progresso?: string }) {
  const versiculos = versiculosDa(biblia, passagem);
  return (
    <article className="biblia-trecho">
      <div className="biblia-trecho-topo">
        <span className="biblia-trilha">{rotulo}</span>
        {progresso && <span className="biblia-progresso">{progresso}</span>}
      </div>
      <h2 className="biblia-ref">{referencia(biblia, passagem)}</h2>
      <p className="biblia-texto">
        {versiculos.map((v) => <span key={v.n} className="biblia-versiculo"><sup>{v.n}</sup>{v.texto} </span>)}
      </p>
    </article>
  );
}

function ParaLer({ biblia, extra, onLi, onAmanha, onMaisTarde }: { biblia: Biblia; extra: boolean; onLi: (l: Leitura[]) => void; onAmanha: () => void; onMaisTarde: () => void }) {
  const g = useLeitura();
  const leitura = leituraDeAgora(biblia, g);
  return (
    <section className="biblia-folha biblia-entra" aria-label="Leitura de agora">
      {extra && <p className="biblia-nota">Você já leu hoje. Estas são as próximas, se quiser adiantar.</p>}
      {leitura.map((l) => {
        const sobre = SOBRE_A_TRILHA[l.trilha];
        const pct = Math.floor((l.indice / Math.max(1, l.total)) * 100);
        return <Trecho key={l.passagem.id} biblia={biblia} passagem={l.passagem} rotulo={`${sobre.emoji} ${sobre.rotulo}`}
          progresso={`${pct}% de ${sobre.nome}${l.ciclo ? ` · ${l.ciclo + 1}ª volta` : ""}`} />;
      })}
      <p className="biblia-fonte">Bíblia Sagrada Ave Maria</p>
      <footer className="biblia-acoes">
        {!extra && <Button variant="ghost" onClick={onMaisTarde} title="Fecha agora; leia quando quiser pela lateral (Palavra do dia)">Ler mais tarde</Button>}
        {!extra && <Button variant="ghost" onClick={onAmanha} title="A mesma leitura volta amanhã">Deixar para amanhã</Button>}
        <Button variant="primary" autoFocus onClick={() => onLi(leitura)}>Li ✓</Button>
      </footer>
    </section>
  );
}

function JaLida({ biblia, onMais, onFechar }: { biblia: Biblia; onMais: () => void; onFechar: () => void }) {
  const g = useLeitura();
  const lidas = lidasHoje(biblia, g, new Date());
  return (
    <section className="biblia-folha biblia-entra" aria-label="Leitura de hoje">
      <p className="biblia-nota is-feita">✓ Leitura de hoje feita</p>
      {lidas.map((p) => <Trecho key={p.id} biblia={biblia} passagem={p} rotulo={`${SOBRE_A_TRILHA[p.trilha].emoji} ${SOBRE_A_TRILHA[p.trilha].rotulo}`} />)}
      <footer className="biblia-acoes">
        <Button variant="ghost" onClick={onMais}>Ler as próximas</Button>
        <Button variant="primary" autoFocus onClick={onFechar}>Fechar</Button>
      </footer>
    </section>
  );
}

function Amem({ onFechar }: { onFechar: () => void }) {
  const g = useLeitura();
  const agora = new Date();
  const seguidos = diasSeguidos(g, agora);
  const hoje = g.historico.filter((h) => h.dia === diaLocal(agora)).flatMap((h) => h.refs);
  return (
    <section className="biblia-folha biblia-entra biblia-amem" aria-label="Leitura concluída">
      <p className="biblia-amem-titulo">Amém 🙏</p>
      <p className="biblia-nota">{hoje.join(" · ")}</p>
      <p className="biblia-nota">🔥 {seguidos} {seguidos === 1 ? "dia" : "dias"} seguidos de leitura</p>
      <footer className="biblia-acoes">
        <Button variant="primary" autoFocus onClick={onFechar}>Começar o dia</Button>
      </footer>
    </section>
  );
}
