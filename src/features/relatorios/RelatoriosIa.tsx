import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, Card, SegmentedControl } from "../../components/ui";
import { AgentIcon } from "../../components/ui/AgentIcon";
import { ReloadIcon } from "../../components/ui/PixelIcon";

interface Dia { data: string; tokens: number; saida: number; cache_lido: number; prompts: number; respostas: number; conversas: number }
interface Fatia { nome: string; provider?: string; tokens: number; prompts: number }
interface Commit { em_ms: number; ia: boolean; linhas: number }
interface RelatorioIa {
  uso: { dias: Dia[]; por_conta: Fatia[]; por_modelo: Fatia[]; por_projeto: Fatia[]; por_hora: number[] };
  commits: { caminho: string; commits: Commit[] }[];
}

const compacto = new Intl.NumberFormat("pt-BR", { notation: "compact", maximumFractionDigits: 1 });
const inteiro = new Intl.NumberFormat("pt-BR");
const fmt = (n: number) => (n < 10_000 ? inteiro.format(n) : compacto.format(n));
/** "16/10" a partir de "2026-10-16". */
const diaCurto = (data: string) => `${data.slice(8, 10)}/${data.slice(5, 7)}`;
const pasta = (caminho: string) => caminho.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || caminho;

/** Barra horizontal com rótulo e valor: as listas por conta, modelo e projeto. */
function Barras({ itens, total, vazio }: { itens: { chave: string; nome: React.ReactNode; valor: number; texto: string }[]; total: number; vazio: string }) {
  if (itens.length === 0) return <p className="text-xs text-text-muted">{vazio}</p>;
  return (
    <ul className="rel-barras">
      {itens.map((item) => (
        <li key={item.chave}>
          <span className="rel-barras-nome">{item.nome}</span>
          <span className="rel-barras-trilho" aria-hidden><span style={{ width: `${total ? Math.min(100, Math.max(2, (item.valor / total) * 100)) : 0}%` }} /></span>
          <span className="rel-barras-valor">{item.texto}</span>
        </li>
      ))}
    </ul>
  );
}

/**
 * Relatórios de uso de IA: tokens e mensagens por dia (lidos dos transcripts de cada conta), commits
 * que a IA assinou nos projetos abertos e onde/quando você mais usa. Tudo calculado neste PC.
 */
export function RelatoriosIa({ projectPaths }: { projectPaths: string[] }) {
  const [dias, setDias] = useState<7 | 30>(7);
  const [dados, setDados] = useState<RelatorioIa | null>(null);
  const [lendo, setLendo] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  const chaveProjetos = projectPaths.join("\n");

  const ler = useCallback(async () => {
    setLendo(true);
    setErro(null);
    try {
      setDados(await invoke<RelatorioIa>("relatorio_ia", { dias, fusoMin: -new Date().getTimezoneOffset(), projetos: chaveProjetos ? chaveProjetos.split("\n") : [] }));
    } catch (motivo) {
      setErro(motivo instanceof Error ? motivo.message : String(motivo));
    } finally {
      setLendo(false);
    }
  }, [dias, chaveProjetos]);
  useEffect(() => { void ler(); }, [ler]);

  const resumo = useMemo(() => {
    if (!dados) return null;
    const fuso = -new Date().getTimezoneOffset() * 60_000;
    const diaDe = (ms: number) => new Date(ms + fuso).toISOString().slice(0, 10);
    const primeiro = dados.uso.dias[0]?.data ?? "";
    const todos = dados.commits.flatMap((projeto) => projeto.commits.map((c) => ({ ...c, dia: diaDe(c.em_ms), projeto: projeto.caminho })))
      .filter((c) => c.dia >= primeiro);
    const ia = todos.filter((c) => c.ia);
    const commitsPorDia = new Map<string, { ia: number; total: number }>();
    for (const c of todos) {
      const atual = commitsPorDia.get(c.dia) ?? { ia: 0, total: 0 };
      atual.total += 1;
      if (c.ia) atual.ia += 1;
      commitsPorDia.set(c.dia, atual);
    }
    const porProjeto = dados.commits.map((projeto) => {
      const doPeriodo = todos.filter((c) => c.projeto === projeto.caminho);
      return { caminho: projeto.caminho, total: doPeriodo.length, ia: doPeriodo.filter((c) => c.ia).length };
    }).filter((p) => p.total > 0).sort((a, b) => b.ia - a.ia || b.total - a.total);
    const hoje = dados.uso.dias[dados.uso.dias.length - 1];
    const tokens = dados.uso.dias.reduce((soma, d) => soma + d.tokens, 0);
    const prompts = dados.uso.dias.reduce((soma, d) => soma + d.prompts, 0);
    const ativos = dados.uso.dias.filter((d) => d.tokens > 0);
    const pico = ativos.reduce<Dia | null>((maior, d) => (!maior || d.tokens > maior.tokens ? d : maior), null);
    return {
      hoje, tokens, prompts, pico, commitsPorDia, porProjeto,
      mediaDia: ativos.length ? tokens / ativos.length : 0,
      porMensagem: prompts ? tokens / prompts : 0,
      commitsIa: ia.length, commitsTotal: todos.length, linhasIa: ia.reduce((soma, c) => soma + c.linhas, 0),
    };
  }, [dados]);

  const maiorDia = Math.max(1, ...(dados?.uso.dias.map((d) => d.tokens) ?? [0]));
  const maiorHora = Math.max(1, ...(dados?.uso.por_hora ?? [0]));
  const horaPico = dados ? dados.uso.por_hora.indexOf(Math.max(...dados.uso.por_hora)) : -1;

  return (
    <div className="settings-grade">
      <Card title="Período" className="settings-largo"
        aside={<Button size="sm" variant="ghost" onClick={() => void ler()} disabled={lendo}>
          <ReloadIcon aria-hidden className={`h-3.5 w-3.5${lendo ? " anexo-girando" : ""}`} />{lendo ? "Lendo…" : "Atualizar"}
        </Button>}>
        <SegmentedControl<7 | 30> options={[{ value: 7, label: "Últimos 7 dias" }, { value: 30, label: "Últimos 30 dias" }]}
          value={dias} onChange={setDias} />
        <p className="text-[11px] text-text-muted">
          Lido das conversas que o Claude e o Codex guardam neste computador, em todas as contas, e do git dos projetos abertos.
          Nada sai daqui. Tokens = o que a IA leu e escreveu; leituras do cache contam à parte porque custam bem menos.
        </p>
        {erro && <p role="alert" className="text-xs text-danger">{erro}</p>}
      </Card>

      {!dados || !resumo ? (
        <Card title="Relatório" className="settings-largo"><p className="text-xs text-text-muted">{lendo ? "Somando o uso… a primeira leitura pode levar alguns segundos." : "Sem dados."}</p></Card>
      ) : (
        <>
          <section className="rel-cartoes settings-largo" aria-label="Resumo">
            <div className="rel-cartao glass" data-tom="accent">
              <span className="rel-cartao-rotulo">Tokens hoje</span>
              <span className="rel-cartao-num">{fmt(resumo.hoje?.tokens ?? 0)}</span>
              <span className="rel-cartao-sub">média de {fmt(Math.round(resumo.mediaDia))} por dia com uso</span>
            </div>
            <div className="rel-cartao glass" data-tom="info">
              <span className="rel-cartao-rotulo">Mensagens hoje</span>
              <span className="rel-cartao-num">{fmt(resumo.hoje?.prompts ?? 0)}</span>
              <span className="rel-cartao-sub">{fmt(resumo.prompts)} no período · {resumo.hoje?.conversas ?? 0} conversas hoje</span>
            </div>
            <div className="rel-cartao glass" data-tom="success">
              <span className="rel-cartao-rotulo">Commits da IA</span>
              <span className="rel-cartao-num">{resumo.commitsIa}</span>
              <span className="rel-cartao-sub">
                de {resumo.commitsTotal} no período{resumo.commitsTotal ? ` · ${Math.round((resumo.commitsIa / resumo.commitsTotal) * 100)}%` : ""}
              </span>
            </div>
            <div className="rel-cartao glass" data-tom="warning">
              <span className="rel-cartao-rotulo">Linhas escritas pela IA</span>
              <span className="rel-cartao-num">{fmt(resumo.linhasIa)}</span>
              <span className="rel-cartao-sub">adicionadas nos commits da IA</span>
            </div>
          </section>

          <Card title="Uso por dia" className="settings-largo"
            aside={resumo.pico && <span className="text-[11px] text-text-muted">Dia mais intenso: {diaCurto(resumo.pico.data)} · {fmt(resumo.pico.tokens)}</span>}>
            <div className="rel-grafico" role="img" aria-label={`Tokens por dia nos últimos ${dias} dias`}>
              {dados.uso.dias.map((d) => {
                const commits = resumo.commitsPorDia.get(d.data);
                return (
                  <div key={d.data} className="rel-coluna"
                    title={`${diaCurto(d.data)}: ${inteiro.format(d.tokens)} tokens · ${d.prompts} mensagens · ${d.conversas} conversas · ${inteiro.format(d.cache_lido)} lidos do cache${commits ? ` · ${commits.ia}/${commits.total} commits da IA` : ""}`}>
                    <span className="rel-coluna-valor">{d.tokens ? fmt(d.tokens) : ""}</span>
                    <span className="rel-coluna-barra" style={{ height: `${(d.tokens / maiorDia) * 100}%` }} />
                    <span className="rel-coluna-commits" aria-hidden>{commits?.ia ? "◆".repeat(Math.min(3, commits.ia)) : ""}</span>
                    <span className="rel-coluna-dia">{dias === 30 && Number(d.data.slice(8)) % 5 !== 0 ? "" : diaCurto(d.data)}</span>
                  </div>
                );
              })}
            </div>
            <p className="text-[11px] text-text-muted">Passe o mouse num dia para ver os números. ◆ = commit feito pela IA naquele dia.</p>
          </Card>

          <Card title="Por conta">
            <Barras total={resumo.tokens} vazio="Nenhum uso no período."
              itens={dados.uso.por_conta.map((f) => ({
                chave: `${f.provider}-${f.nome}`, valor: f.tokens, texto: `${fmt(f.tokens)} · ${f.prompts} msg`,
                nome: <>{f.provider && <AgentIcon provider={f.provider} size={12} className="shrink-0" />}<span className="truncate">{f.nome}</span></>,
              }))} />
          </Card>

          <Card title="Por modelo">
            <Barras total={resumo.tokens} vazio="Nenhum uso no período."
              itens={dados.uso.por_modelo.map((f) => ({ chave: f.nome, valor: f.tokens, texto: fmt(f.tokens), nome: <span className="truncate">{f.nome}</span> }))} />
          </Card>

          <Card title="Por projeto">
            <Barras total={resumo.tokens} vazio="Nenhum uso no período."
              itens={dados.uso.por_projeto.slice(0, 8).map((f) => ({ chave: f.nome, valor: f.tokens, texto: `${fmt(f.tokens)} · ${f.prompts} msg`, nome: <span className="truncate">{f.nome}</span> }))} />
          </Card>

          <Card title="Commits por projeto">
            <Barras total={Math.max(1, ...resumo.porProjeto.map((p) => p.total))} vazio="Nenhum commit nos projetos abertos no período."
              itens={resumo.porProjeto.slice(0, 8).map((p) => ({
                chave: p.caminho, valor: p.ia, texto: `${p.ia} de ${p.total}`, nome: <span className="truncate" title={p.caminho}>{pasta(p.caminho)}</span>,
              }))} />
          </Card>

          <Card title="Quando você mais usa" className="settings-largo"
            aside={horaPico >= 0 && dados.uso.por_hora[horaPico] > 0 && <span className="text-[11px] text-text-muted">Pico às {horaPico}h</span>}>
            <div className="rel-horas" role="img" aria-label="Mensagens por hora do dia">
              {dados.uso.por_hora.map((n, hora) => (
                <div key={hora} className="rel-hora" title={`${hora}h: ${n} mensagens`}>
                  <span className="rel-hora-barra" style={{ height: `${(n / maiorHora) * 100}%` }} />
                  <span className="rel-hora-rotulo">{hora % 3 === 0 ? `${hora}h` : ""}</span>
                </div>
              ))}
            </div>
            <p className="text-[11px] text-text-muted">
              Em média {fmt(Math.round(resumo.porMensagem))} tokens por mensagem sua no período.
            </p>
          </Card>
        </>
      )}
    </div>
  );
}
