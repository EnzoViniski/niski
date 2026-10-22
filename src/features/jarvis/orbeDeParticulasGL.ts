// Fork: orbe de partículas. Desenho em WebGL2: todas as partículas num único buffer e numa única
// chamada de desenho; o movimento inteiro (respirar, tremer com a voz, ondas que saem do centro,
// redemoinho pensando, formação ao abrir) é calculado no shader de vértice. Por quadro o JS só
// manda uns poucos números (uniforms) — nada de recalcular partícula na CPU.
import { FLOATS_POR_PARTICULA } from "./particulasDaLogo";
import { TAMANHO_DO_HISTORICO } from "./nivelDaVoz";

export interface QuadroDoOrbe {
  tempo: number;
  /** 0 → espalhadas, 1 → logo formada. */
  forma: number;
  fala: number;
  mic: number;
  pensar: number;
  pausa: number;
  erro: number;
  /** "Conectando" (estado iniciando), 0..1. */
  iniciar: number;
  historico: Float32Array;
}

const VERTICE = `#version 300 es
in vec2 aCasa;
in vec3 aCor;
in float aSemente;
in float aTipo;
in float aBrilho;
uniform float uTempo, uForma, uFala, uMic, uPensar, uPausa, uErro, uIniciar, uPonto, uEscala, uMovimento;
uniform float uHist[${TAMANHO_DO_HISTORICO}];
out vec3 vCor;
out float vAlfa;

vec2 girar(vec2 p, float a) { float c = cos(a), s = sin(a); return vec2(c * p.x - s * p.y, s * p.x + c * p.y); }
float acaso(float n) { return fract(sin(n) * 43758.5453); }

void main() {
  float s = aSemente;
  vec2 p = aCasa;
  float d = length(p);
  vec2 dir = d > 0.0001 ? p / d : vec2(0.0);
  float poeira = step(1.5, aTipo) * (1.0 - step(2.5, aTipo));
  float halo = step(2.5, aTipo);
  float energia = max(uFala, uMic * 0.6);

  // Onda: a energia que estava no centro há k amostras chega agora ao raio k/N.
  float k = clamp(d, 0.0, 1.0) * ${TAMANHO_DO_HISTORICO - 1}.0;
  int i0 = int(floor(k));
  int i1 = min(i0 + 1, ${TAMANHO_DO_HISTORICO - 1});
  float onda = mix(uHist[i0], uHist[i1], fract(k)) * uMovimento;

  // Respira devagar (menos em pausa) e incha com a onda que passa.
  float respiro = sin(uTempo * 1.1) * 0.014 * (1.0 - uPausa * 0.7) * uMovimento;
  // Dois lados da conversa com gestos diferentes: a voz do Enzo contrai e aperta a logo (escutando),
  // a voz do Jarvis expande em ondas (falando).
  float contrai = uMic * 0.075 * (1.0 - uFala);
  // Sem som também há vida: "conectando" pulsa de leve, o erro bate devagar como um coração.
  float conectando = uIniciar * 0.03 * sin(uTempo * 2.4 - d * 3.0);
  float batida = uErro * 0.035 * pow(max(0.0, sin(uTempo * 2.2)), 6.0);
  p *= 1.0 + respiro + onda * 0.16 - contrai + conectando + batida;

  // Vibração própria de cada partícula: mais rápida e mais larga com a voz.
  float f = 1.2 + s * 2.2 + energia * 9.0 + uMic * 5.0;
  vec2 tremor = vec2(sin(uTempo * f + s * 31.0), cos(uTempo * f * 0.87 + s * 17.0));
  p += tremor * (0.005 + energia * 0.045 + onda * 0.03) * uMovimento * (1.0 - halo * 0.7);

  // Contorno ondulado quando o Jarvis fala (lobos que giram, como uma voz sintetizada).
  float ang = atan(p.y, p.x);
  float lobos = sin(ang * 5.0 + uTempo * 3.1) * sin(ang * 3.0 - uTempo * 2.3);
  p += dir * lobos * uFala * 0.09 * uMovimento * (0.4 + d);

  // Pensando: redemoinho que torce o miolo mais que a borda.
  float torcao = uPensar * (0.35 * sin(uTempo * 0.9 + d * 5.0) + 0.3) * (1.0 - d * 0.45);
  p = girar(p, torcao * 0.7 * uMovimento);

  // Poeira em órbita lenta, que se afasta com a voz.
  if (poeira > 0.5) {
    p = girar(p, uTempo * (0.06 + s * 0.1) * (1.0 + uPensar * 2.0) * uMovimento);
    p *= 1.0 + energia * 0.22 * acaso(s * 9.1);
  }

  // Formação: cada partícula sai de um ponto espalhado e chega com um atraso próprio.
  vec2 longe = (vec2(acaso(s * 13.7), acaso(s * 7.3)) - 0.5) * 3.4;
  float t = clamp(uForma * 1.3 - acaso(s * 3.1) * 0.3, 0.0, 1.0);
  t = t * t * (3.0 - 2.0 * t);
  p = mix(longe, p, t);

  gl_Position = vec4(p * uEscala, 0.0, 1.0);
  float tam = uPonto * mix(1.0, 0.75, poeira) * mix(1.0, 7.0, halo) * (0.65 + 0.7 * acaso(s * 5.3)) * (1.0 + onda * 0.7 + uFala * 0.3);
  gl_PointSize = max(1.0, tam);

  vec3 cor = aCor;
  float cinza = dot(cor, vec3(0.299, 0.587, 0.114));
  // Pensando: puxa para o violeta da logo e passa uma faixa de luz girando, como "processando".
  cor = mix(cor, mix(cor, vec3(0.72, 0.58, 1.0), 0.55), uPensar * 0.6);
  cor += vec3(0.85, 0.8, 1.0) * uPensar * 0.55 * pow(max(0.0, sin(ang - uTempo * 1.6)), 10.0);
  cor = mix(cor, vec3(cinza * 0.75), uPausa * 0.85);
  cor = mix(cor, vec3(1.0, 0.32, 0.36), uErro * 0.75);
  cor += vec3(onda * 0.45 + uFala * 0.12);
  // Escutando: um toque ciano no contorno enquanto o Enzo fala.
  cor += vec3(0.1, 0.35, 0.45) * uMic * (1.0 - uFala) * step(0.5, aTipo) * (1.0 - step(1.5, aTipo));
  // Conectando: duas faixas azuis girando; erro: brilho que acompanha a batida.
  cor += vec3(0.45, 0.7, 1.0) * uIniciar * 0.5 * pow(max(0.0, sin(ang * 2.0 - uTempo * 2.2)), 12.0);
  cor *= 1.0 + uErro * 0.5 * pow(max(0.0, sin(uTempo * 2.2)), 6.0);

  float cintila = 0.72 + 0.28 * sin(uTempo * (1.8 + s * 3.0) + s * 40.0);
  float alfaPoeira = mix(1.0, 0.25 + energia * 0.6 + uPensar * 0.2, poeira);
  float alfaHalo = mix(1.0, 1.0 + energia * 2.2 + uPensar * 0.8, halo);
  vAlfa = aBrilho * cintila * alfaPoeira * alfaHalo * (1.0 - uPausa * 0.4) * t;
  vCor = cor;
}`;

const FRAGMENTO = `#version 300 es
precision mediump float;
in vec3 vCor;
in float vAlfa;
uniform float uIntensidade;
out vec4 saida;
void main() {
  vec2 q = gl_PointCoord * 2.0 - 1.0;
  float r2 = dot(q, q);
  if (r2 > 1.0) discard;
  float a = exp(-r2 * 3.2) * vAlfa * uIntensidade;
  saida = vec4(vCor * a, a);
}`;

export interface DesenhistaDoOrbe {
  desenhar(quadro: QuadroDoOrbe): void;
  redimensionar(ladoCss: number, escala: number): void;
  destruir(): void;
}

function compilar(gl: WebGL2RenderingContext, tipo: number, fonte: string): WebGLShader {
  const shader = gl.createShader(tipo);
  if (!shader) throw new Error("shader");
  gl.shaderSource(shader, fonte);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(shader) ?? "";
    gl.deleteShader(shader);
    throw new Error(`shader não compilou: ${log}`);
  }
  return shader;
}

/** Prepara o WebGL2 no canvas. Devolve `null` se não houver WebGL2 (o componente cai no orbe 2D). */
export function criarDesenhista(canvas: HTMLCanvasElement, particulas: Float32Array, opcoes: { lado: number; pontoBase: number; escala: number }): DesenhistaDoOrbe | null {
  const gl = canvas.getContext("webgl2", { alpha: true, premultipliedAlpha: true, antialias: false, depth: false, stencil: false, powerPreference: "low-power" });
  if (!gl) return null;
  const vs = compilar(gl, gl.VERTEX_SHADER, VERTICE);
  const fs = compilar(gl, gl.FRAGMENT_SHADER, FRAGMENTO);
  const programa = gl.createProgram();
  if (!programa) return null;
  gl.attachShader(programa, vs);
  gl.attachShader(programa, fs);
  gl.linkProgram(programa);
  gl.deleteShader(vs);
  gl.deleteShader(fs);
  if (!gl.getProgramParameter(programa, gl.LINK_STATUS)) throw new Error(`programa não ligou: ${gl.getProgramInfoLog(programa) ?? ""}`);

  const vao = gl.createVertexArray();
  const buffer = gl.createBuffer();
  gl.bindVertexArray(vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  gl.bufferData(gl.ARRAY_BUFFER, particulas, gl.STATIC_DRAW);
  const passo = FLOATS_POR_PARTICULA * 4;
  const atributo = (nome: string, tamanho: number, deslocamento: number) => {
    const local = gl.getAttribLocation(programa, nome);
    if (local < 0) return;
    gl.enableVertexAttribArray(local);
    gl.vertexAttribPointer(local, tamanho, gl.FLOAT, false, passo, deslocamento * 4);
  };
  atributo("aCasa", 2, 0);
  atributo("aCor", 3, 2);
  atributo("aSemente", 1, 5);
  atributo("aTipo", 1, 6);
  atributo("aBrilho", 1, 7);
  gl.bindVertexArray(null);

  const u = (nome: string) => gl.getUniformLocation(programa, nome);
  const locais = {
    tempo: u("uTempo"), forma: u("uForma"), fala: u("uFala"), mic: u("uMic"), pensar: u("uPensar"),
    pausa: u("uPausa"), erro: u("uErro"), iniciar: u("uIniciar"), ponto: u("uPonto"), escala: u("uEscala"), movimento: u("uMovimento"),
    historico: u("uHist"), intensidade: u("uIntensidade"),
  };
  const total = particulas.length / FLOATS_POR_PARTICULA;
  const devagar = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
  let ponto = 1;

  const redimensionar = (ladoCss: number, escala: number) => {
    canvas.width = Math.round(ladoCss * escala);
    canvas.height = Math.round(ladoCss * escala);
    gl.viewport(0, 0, canvas.width, canvas.height);
    // Ponto proporcional ao lado (a bolinha usa pontos relativamente maiores, com menos partículas).
    ponto = opcoes.pontoBase * (ladoCss / 200) * escala;
  };
  // Até 1,5× de densidade: as partículas são pontos suaves, a diferença para 2× não aparece e o canvas
  // fica com ~44% menos pixels para pintar e compor a cada quadro.
  redimensionar(opcoes.lado, Math.min(1.5, window.devicePixelRatio || 1));

  return {
    redimensionar,
    desenhar(q) {
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.useProgram(programa);
      gl.enable(gl.BLEND);
      // Aditivo: onde as partículas se juntam, brilha — o mesmo efeito de luz da logo.
      gl.blendFunc(gl.ONE, gl.ONE);
      gl.uniform1f(locais.tempo, q.tempo);
      gl.uniform1f(locais.forma, q.forma);
      gl.uniform1f(locais.fala, q.fala);
      gl.uniform1f(locais.mic, q.mic);
      gl.uniform1f(locais.pensar, q.pensar);
      gl.uniform1f(locais.pausa, q.pausa);
      gl.uniform1f(locais.erro, q.erro);
      gl.uniform1f(locais.iniciar, q.iniciar);
      gl.uniform1f(locais.ponto, ponto);
      gl.uniform1f(locais.escala, opcoes.escala);
      gl.uniform1f(locais.movimento, devagar ? 0.3 : 1);
      gl.uniform1f(locais.intensidade, 0.62);
      gl.uniform1fv(locais.historico, q.historico);
      gl.bindVertexArray(vao);
      gl.drawArrays(gl.POINTS, 0, total);
      gl.bindVertexArray(null);
    },
    destruir() {
      gl.deleteBuffer(buffer);
      gl.deleteVertexArray(vao);
      gl.deleteProgram(programa);
      // Libera o contexto na hora: alternar painel ↔ bolinha não acumula contextos WebGL.
      gl.getExtension("WEBGL_lose_context")?.loseContext();
    },
  };
}
