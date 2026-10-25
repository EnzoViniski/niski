// Fork (Bíblia do dia): cartão em Configurações → Manhã — ligar, trilhas, tamanho da letra e onde parou.
import { Button, Card, SegmentedControl } from "../../components/ui";
import { useBiblia } from "./arquivo";
import { passagemAtual, referencia, SOBRE_A_TRILHA, TRILHAS, type Trilha } from "./biblia";
import { diasSeguidos, salvarAjustesDaBiblia, useLeitura } from "./leitura";
import { abrirLeituraAgora } from "./useLeituraDoDia";

export function AjustesDaBiblia() {
  const { ajustes, marcadores, historico } = useLeitura();
  const g = useLeitura();
  const arquivo = useBiblia();
  const alternar = (t: Trilha) => salvarAjustesDaBiblia({ trilhas: ajustes.trilhas.includes(t) ? ajustes.trilhas.filter((x) => x !== t) : TRILHAS.filter((x) => x === t || ajustes.trilhas.includes(x)) });

  return (
    <Card title="Palavra do dia" aside={<Button size="sm" variant="primary" onClick={abrirLeituraAgora}>Ler agora</Button>}>
      <p className="text-sm text-text-secondary">
        Logo depois do aquecimento da manhã, uma passagem da Bíblia Sagrada (tradução Ave Maria, católica, 73 livros) por trilha
        escolhida. Nunca repete: cada trilha continua de onde você parou. "Deixar para amanhã" e "Ler mais tarde" guardam a mesma
        leitura; a qualquer hora ela abre pela lateral (Palavra do dia).
      </p>
      <label className="flex items-center gap-2 text-sm text-text-primary">
        <input type="checkbox" checked={ajustes.ativa} onChange={(e) => salvarAjustesDaBiblia({ ativa: e.target.checked })} />
        Mostrar a Palavra do dia todos os dias
      </label>
      <div className={`space-y-2 ${ajustes.ativa ? "" : "pointer-events-none opacity-50"}`}>
        {TRILHAS.map((t) => {
          const sobre = SOBRE_A_TRILHA[t];
          const biblia = arquivo.fase === "pronta" ? arquivo.biblia : null;
          const atual = biblia ? passagemAtual(biblia, t, marcadores[t]) : null;
          return (
            <label key={t} className="flex items-start gap-2 text-sm text-text-primary">
              <input type="checkbox" className="mt-1" checked={ajustes.trilhas.includes(t)} onChange={() => alternar(t)} />
              <span>
                {sobre.emoji} {sobre.nome}
                <span className="block text-xs text-text-muted">
                  {sobre.dica}
                  {biblia && atual && marcadores[t] ? ` Próxima: ${referencia(biblia, atual.passagem)} (${Math.floor((atual.indice / atual.total) * 100)}% lido).` : ""}
                </span>
              </span>
            </label>
          );
        })}
        {!ajustes.trilhas.length && <p className="text-xs text-warning">Nenhuma trilha escolhida: a Palavra do dia não aparece.</p>}
        <SegmentedControl label="Tamanho da letra" value={ajustes.letra} onChange={(letra) => salvarAjustesDaBiblia({ letra })}
          options={[{ value: "normal", label: "Normal" }, { value: "grande", label: "Grande" }]} />
      </div>
      {historico.length > 0 && (
        <p className="text-xs text-text-muted">🔥 {diasSeguidos(g, new Date())} dias seguidos · {new Set(historico.map((h) => h.dia)).size} dias de leitura · {historico.reduce((s, h) => s + h.ids.length, 0)} passagens lidas</p>
      )}
      {arquivo.fase === "erro" && <p className="text-xs text-warning">A Bíblia ainda não foi baixada ({arquivo.erro}). Ela baixa sozinha na primeira leitura.</p>}
    </Card>
  );
}
