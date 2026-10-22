// Fork: orbe de partículas. Transforma o nível cru da voz (`nivel` do auxiliar Swift, RMS em dB
// normalizado 0..1, 20 vezes por segundo) no que o orbe precisa por quadro:
// - portão de ruído com piso adaptativo no microfone (o ruído da sala vira 0, a fala vira 0..1);
// - ataque rápido e soltura lenta, por tempo (igual em 60 ou 120 Hz);
// - um histórico curto da energia, que o shader espalha do centro para fora como ondas.
// Puro, sem DOM: testado em `src/test/OrbeDeParticulas.test.tsx`.

/** Quantas amostras de histórico vão para o shader (`uHist`). */
export const TAMANHO_DO_HISTORICO = 32;
/** Amostras por segundo no histórico: a onda leva 32/40 = 0,8 s do centro até a borda. */
const AMOSTRAS_POR_SEGUNDO = 40;

const limitar = (v: number) => (v < 0 ? 0 : v > 1 ? 1 : v);

/** Média exponencial com constante de tempo `tau` (segundos), independente da taxa de quadros. */
function seguir(atual: number, alvo: number, dt: number, tauSubindo: number, tauDescendo: number): number {
  const tau = alvo > atual ? tauSubindo : tauDescendo;
  return atual + (alvo - atual) * (1 - Math.exp(-dt / tau));
}

export class SuavizadorDeVoz {
  /** Nível do ruído de fundo do microfone (mesma escala do nível cru). */
  piso = 0.25;
  mic = 0;
  fala = 0;
  /** `historico[0]` é o agora (centro do orbe); o último é o mais antigo (borda). */
  readonly historico = new Float32Array(TAMANHO_DO_HISTORICO);
  private acumulado = 0;

  passo(cru: { mic: number; fala: number }, dt: number): void {
    const micCru = limitar(Number.isFinite(cru.mic) ? cru.mic : 0);
    const falaCru = limitar(Number.isFinite(cru.fala) ? cru.fala : 0);
    // Piso: desce rápido até o silêncio e sobe devagar (barulho constante deixa de contar como voz).
    this.piso = micCru < this.piso ? this.piso + (micCru - this.piso) * 0.3 : Math.min(0.7, this.piso + dt * 0.015);
    const micAberto = limitar((micCru - this.piso - 0.04) / 0.4);
    // A fala do Jarvis vem da síntese, sem ruído: só tira o rabo baixo.
    const falaAberta = limitar((falaCru - 0.3) / 0.55);
    this.mic = seguir(this.mic, micAberto, dt, 0.04, 0.2);
    this.fala = seguir(this.fala, falaAberta, dt, 0.03, 0.16);

    this.acumulado += dt;
    const periodo = 1 / AMOSTRAS_POR_SEGUNDO;
    // As ondas que saem do centro são da voz do Jarvis; a do Enzo entra fraca (ela contrai a logo no shader).
    const energia = Math.max(this.fala, this.mic * 0.35);
    // Atraso grande (janela escondida e voltando) não precisa empurrar mais que o histórico todo.
    let empurrar = Math.min(TAMANHO_DO_HISTORICO, Math.floor(this.acumulado / periodo));
    this.acumulado -= Math.floor(this.acumulado / periodo) * periodo;
    while (empurrar-- > 0) {
      this.historico.copyWithin(1, 0, TAMANHO_DO_HISTORICO - 1);
      this.historico[0] = energia;
    }
  }
}

/** Valor que persegue um alvo (0..1) com constante de tempo fixa: troca de estado sem salto. */
export function aproximar(atual: number, alvo: number, dt: number, tau = 0.35): number {
  return seguir(atual, alvo, dt, tau, tau);
}
