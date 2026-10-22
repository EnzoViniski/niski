// Fork (Manhã): seção "Manhã" das Configurações — ligar, horário, quantidade, dificuldade e temas.
import { Button, Card, Input, SegmentedControl, Slider } from "../../components/ui";
import { EscolhaDaIa } from "../roadmaps/comuns";
import { EMOJI_DO_TEMA, ROTULO_DO_TEMA, TEMAS_GERADOS, type Tema } from "./gerador";
import { diaLocal, salvarAjustesDaManha, sequencia, useManha, type AjustesDaManha as Ajustes } from "./rotina";
import { abrirRotinaAgora } from "./useRotinaDaManha";

const EXTRAS: { tema: Tema; dica: string }[] = [
  { tema: "roadmaps", dica: "Cards dos seus Roadmaps de estudo, com as respostas dos outros cards como alternativas. Conta como revisão." },
  { tema: "livre", dica: "A IA escreve questões sobre o assunto que você quiser (inglês, anatomia, direito, história…)." },
];

export function AjustesDaManha() {
  const { ajustes, historico } = useManha();
  const mudar = (m: Partial<Ajustes>) => salvarAjustesDaManha(m);
  const alternarTema = (tema: Tema) => mudar({ temas: ajustes.temas.includes(tema) ? ajustes.temas.filter((t) => t !== tema) : [...ajustes.temas, tema] });
  const feitos = historico.length;
  const taxa = feitos ? Math.round((historico.reduce((s, d) => s + d.acertos, 0) / Math.max(1, historico.reduce((s, d) => s + d.total, 0))) * 100) : 0;

  return (
    <div className="space-y-4">
      <Card title="Aquecimento da manhã" aside={<Button size="sm" variant="primary" onClick={abrirRotinaAgora}>Fazer agora</Button>}>
        <p className="text-sm text-text-secondary">
          Todo dia, a partir do horário escolhido, a primeira coisa que aparece ao abrir o Niski são algumas questões rápidas
          de múltipla escolha para acordar o cérebro. Se o computador ligar depois do horário, elas aparecem assim que o app abrir.
        </p>
        <label className="flex items-center gap-2 text-sm text-text-primary">
          <input type="checkbox" checked={ajustes.ativo} onChange={(e) => mudar({ ativo: e.target.checked })} />
          Mostrar o aquecimento todos os dias
        </label>
        <div className={`grid gap-4 sm:grid-cols-2 ${ajustes.ativo ? "" : "pointer-events-none opacity-50"}`}>
          <label className="space-y-1 text-sm text-text-primary">
            <span>A partir de que horas</span>
            <Input type="time" value={ajustes.horario} onChange={(e) => e.target.value && mudar({ horario: e.target.value })} className="w-32" />
          </label>
          <Slider id="manha-quantidade" label="Questões por dia" min={1} max={20} step={1} value={ajustes.quantidade} onChange={(quantidade) => mudar({ quantidade })} />
        </div>
        <SegmentedControl label="Dificuldade" value={ajustes.dificuldade} onChange={(dificuldade) => mudar({ dificuldade })}
          description={ajustes.dificuldade === "adaptativa" ? "Sobe quando você acerta quase tudo e desce quando erra muito (últimos 5 dias)." : undefined}
          options={[{ value: "facil", label: "Fácil" }, { value: "medio", label: "Média" }, { value: "dificil", label: "Difícil" }, { value: "adaptativa", label: "Se adapta a mim" }]} />
        <label className="flex items-center gap-2 text-sm text-text-primary">
          <input type="checkbox" checked={ajustes.tarefasNoFim} onChange={(e) => mudar({ tarefasNoFim: e.target.checked })} />
          No fim, mostrar as minhas tarefas de hoje
        </label>
        {feitos > 0 && (
          <p className="text-xs text-text-muted">🔥 {sequencia(historico, diaLocal(new Date()))} dias seguidos · {feitos} sessões feitas · {taxa}% de acerto no total</p>
        )}
      </Card>

      <Card title="O que cai">
        <p className="text-xs text-text-muted">Escolha um ou mais. As questões do dia se dividem entre os escolhidos.</p>
        <div className="flex flex-wrap gap-2">
          {TEMAS_GERADOS.map((tema) => (
            <button key={tema} type="button" aria-pressed={ajustes.temas.includes(tema)} onClick={() => alternarTema(tema)}
              className={`btn-${ajustes.temas.includes(tema) ? "primary" : "glass"} px-3 py-2 text-xs`}>
              {EMOJI_DO_TEMA[tema]} {ROTULO_DO_TEMA[tema]}
            </button>
          ))}
        </div>
        <div className="space-y-3 pt-1">
          {EXTRAS.map(({ tema, dica }) => (
            <div key={tema} className="space-y-2">
              <label className="flex items-start gap-2 text-sm text-text-primary">
                <input type="checkbox" className="mt-1" checked={ajustes.temas.includes(tema)} onChange={() => alternarTema(tema)} />
                <span>{EMOJI_DO_TEMA[tema]} {ROTULO_DO_TEMA[tema]}<span className="block text-xs text-text-muted">{dica}</span></span>
              </label>
              {tema === "livre" && ajustes.temas.includes("livre") && (
                <div className="ml-6 space-y-2">
                  <Input value={ajustes.temaLivre} onChange={(e) => mudar({ temaLivre: e.target.value })} placeholder="Ex.: vocabulário de inglês, farmacologia, capitais do mundo" />
                  <EscolhaDaIa valor={ajustes.ia} onChange={(ia) => mudar({ ia })} />
                </div>
              )}
            </div>
          ))}
        </div>
        {!ajustes.temas.length && <p className="text-xs text-warning">Nenhum tema escolhido: o aquecimento não aparece.</p>}
        <p className="text-xs text-text-muted">Se a revisão ou o tema livre não tiverem questões no dia (sem cards, IA indisponível), entram questões de matemática no lugar.</p>
      </Card>
    </div>
  );
}
