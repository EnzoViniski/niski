import { CheckIcon as Check, ChevronDownIcon as ChevronDown, SearchIcon as Search } from "./PixelIcon";
import { useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { ReactNode } from "react";
import type { FieldSize } from "./Input";

export interface SelectOption<T extends string | number> {
  value: T;
  label: string;
  /** Linha secundária dentro da opção (peso do modelo, explicação curta). */
  hint?: string;
  /** Marca à esquerda do rótulo (logo do provider). Some da busca — é só visual. */
  icon?: ReactNode;
  disabled?: boolean;
}

export interface SelectProps<T extends string | number> {
  id?: string;
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  size?: FieldSize;
  disabled?: boolean;
  className?: string;
  /** Texto quando `value` não casa com nenhuma opção. */
  placeholder?: string;
  /** Campo de busca no topo da lista. Ligue quando passar de ~15 opções. */
  searchable?: boolean;
  /** Texto do campo de busca. */
  searchPlaceholder?: string;
}

const SIZE: Record<FieldSize, string> = {
  sm: "px-2.5 py-1.5 text-xs",
  md: "px-3 py-2 text-sm",
};

/**
 * Select próprio, não `<select>` nativo.
 *
 * O motivo é simples: a lista de um `<select>` é desenhada pelo **sistema
 * operacional**, não pelo WebView. Nenhuma regra de CSS alcança ela — no
 * Windows sai um menu branco quadrado, ignorando tema, vidro e o roxo do app.
 * A única forma de padronizar é desenhar a lista em HTML.
 *
 * O que isso obriga a reimplementar (e está aqui): papéis ARIA de
 * combobox/listbox, navegação por teclado, fechar no Esc e no clique fora.
 */
export function Select<T extends string | number>({
  id,
  value,
  options,
  onChange,
  size = "md",
  disabled,
  className,
  placeholder = "Selecione…",
  searchable = false,
  searchPlaceholder = "Pesquisar…",
}: SelectProps<T>) {
  const [aberto, setAberto] = useState(false);
  const [ativo, setAtivo] = useState(0);
  const [busca, setBusca] = useState("");
  const [paraCima, setParaCima] = useState(false);
  // Fork: a lista vai para o `body` com posição fixa, medida do botão. Dentro do painel ela era
  // cortada pelo `overflow` dele (o seletor de conta/modelo/esforço do chat sumia atrás do painel de
  // baixo) e tinha a largura do botão — "Esforço padrão" quebrava em duas linhas.
  const [posicao, setPosicao] = useState<{ left: number; top?: number; bottom?: number; minWidth: number; maxHeight: number } | null>(null);
  const popRef = useRef<HTMLDivElement>(null);
  const raizRef = useRef<HTMLDivElement>(null);
  const listaRef = useRef<HTMLUListElement>(null);
  const buscaRef = useRef<HTMLInputElement>(null);
  const autoId = useId();
  const listaId = `${id ?? autoId}-lista`;

  const selecionado = options.find((o) => o.value === value);

  // Casa sem acento: "voce" acha "Você". Nome de fonte quase sempre é ASCII,
  // mas o Select é genérico e o custo é uma linha.
  const visiveis = useMemo(() => {
    const termo = busca.trim().normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
    if (!searchable || !termo) return options;
    return options.filter((o) =>
      `${o.label} ${o.hint ?? ""}`
        .normalize("NFD")
        .replace(/\p{Diacritic}/gu, "")
        .toLowerCase()
        .includes(termo)
    );
  }, [options, busca, searchable]);

  const selecionadoIdx = visiveis.findIndex((o) => o.value === value);

  // Fecha ao clicar fora. Sem isto o popover fica preso quando o usuário já
  // seguiu para outra coisa.
  useEffect(() => {
    if (!aberto) return;
    function onDown(e: PointerEvent) {
      const alvo = e.target as Node;
      if (!raizRef.current?.contains(alvo) && !popRef.current?.contains(alvo)) setAberto(false);
    }
    document.addEventListener("pointerdown", onDown);
    return () => document.removeEventListener("pointerdown", onDown);
  }, [aberto]);

  // Abre para cima quando não cabe embaixo — a lista tem até 240px e vários
  // selects ficam na metade de baixo da página.
  useLayoutEffect(() => {
    if (!aberto) {
      // Zera a busca ao fechar: reabrir com o filtro de antes escondendo a
      // opção marcada dá a impressão de que a lista encolheu sozinha.
      setBusca("");
      return;
    }
    medir();
    setAtivo(selecionadoIdx >= 0 ? selecionadoIdx : 0);
    buscaRef.current?.focus();
    // `selecionadoIdx` de propósito fora das deps: ele muda a cada tecla
    // digitada (a lista filtra), e reposicionar o ativo aí atrapalharia a
    // navegação por seta durante a busca.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [aberto]);

  function medir() {
    const r = raizRef.current?.getBoundingClientRect();
    if (!r) return;
    const embaixo = window.innerHeight - r.bottom - 8;
    const cima = r.top > embaixo && embaixo < 260;
    setParaCima(cima);
    const espaco = Math.max(120, (cima ? r.top : embaixo) - 12);
    const left = Math.max(8, Math.min(r.left, window.innerWidth - Math.max(r.width, 200) - 8));
    setPosicao(cima
      ? { left, bottom: window.innerHeight - r.top + 6, minWidth: r.width, maxHeight: espaco }
      : { left, top: r.bottom + 6, minWidth: r.width, maxHeight: espaco });
  }

  // Fork: a lista é fixa na tela; rolar ou redimensionar com ela aberta realinha ao botão.
  useEffect(() => {
    if (!aberto) return;
    const realinhar = (e: Event) => { if (!popRef.current?.contains(e.target as Node)) medir(); };
    window.addEventListener("resize", realinhar);
    window.addEventListener("scroll", realinhar, true);
    return () => { window.removeEventListener("resize", realinhar); window.removeEventListener("scroll", realinhar, true); };
  }, [aberto]);

  // Digitar refaz a lista: o ativo tem de voltar para o topo, senão aponta
  // para um índice que já não existe.
  useEffect(() => {
    setAtivo(0);
  }, [busca]);

  // Mantém a opção ativa visível durante a navegação por teclado.
  // `?.()` na chamada: `scrollIntoView` não existe no jsdom, e um TypeError
  // aqui derruba o componente inteiro por causa de um detalhe cosmético.
  useEffect(() => {
    if (!aberto) return;
    listaRef.current?.children[ativo]?.scrollIntoView?.({ block: "nearest" });
  }, [aberto, ativo]);

  function proximoHabilitado(de: number, passo: number) {
    if (visiveis.length === 0) return de;
    for (let i = 1; i <= visiveis.length; i++) {
      const idx = (de + passo * i + visiveis.length * 2) % visiveis.length;
      if (!visiveis[idx].disabled) return idx;
    }
    return de;
  }

  function escolher(idx: number) {
    const o = visiveis[idx];
    if (!o || o.disabled) return;
    onChange(o.value);
    setAberto(false);
  }

  function onKeyDown(e: React.KeyboardEvent) {
    if (disabled) return;

    if (!aberto) {
      if (["Enter", " ", "ArrowDown", "ArrowUp"].includes(e.key)) {
        e.preventDefault();
        setAberto(true);
      }
      return;
    }

    switch (e.key) {
      case "Escape":
        e.preventDefault();
        setAberto(false);
        break;
      case "ArrowDown":
        e.preventDefault();
        setAtivo((i) => proximoHabilitado(i, 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setAtivo((i) => proximoHabilitado(i, -1));
        break;
      case "Home":
        e.preventDefault();
        setAtivo(proximoHabilitado(visiveis.length - 1, 1));
        break;
      case "End":
        e.preventDefault();
        setAtivo(proximoHabilitado(0, -1));
        break;
      case "Enter":
      case " ":
        e.preventDefault();
        escolher(ativo);
        break;
      case "Tab":
        setAberto(false);
        break;
    }
  }

  return (
    <div ref={raizRef} className={["relative", className].filter(Boolean).join(" ")}>
      <button
        id={id}
        type="button"
        role="combobox"
        aria-expanded={aberto}
        aria-haspopup="listbox"
        aria-controls={aberto ? listaId : undefined}
        aria-activedescendant={aberto ? `${listaId}-${ativo}` : undefined}
        disabled={disabled}
        onClick={() => setAberto((v) => !v)}
        onKeyDown={onKeyDown}
        className={[
          "field flex items-center justify-between gap-2 text-left",
          SIZE[size],
          aberto ? "select-aberto" : "",
        ].join(" ")}
      >
        <span className={["flex min-w-0 items-center gap-1.5", selecionado ? "" : "text-text-muted"].join(" ")}>
          {selecionado?.icon}
          <span className="truncate">{selecionado?.label ?? placeholder}</span>
        </span>
        <ChevronDown
          aria-hidden="true"
          className={[
            "w-4 h-4 shrink-0 text-text-muted transition-transform",
            aberto ? "rotate-180" : "",
          ].join(" ")}
        />
      </button>

      {aberto && posicao && createPortal(
        <div
          ref={popRef}
          data-lado={paraCima ? "cima" : "baixo"}
          style={{ position: "fixed", left: posicao.left, top: posicao.top, bottom: posicao.bottom, minWidth: posicao.minWidth, maxWidth: 360 }}
          className="popover select-pop z-[200] p-1"
        >
          {searchable && (
            <div className="relative mb-1">
              <Search
                aria-hidden="true"
                className="absolute left-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-text-muted"
              />
              <input
                ref={buscaRef}
                type="text"
                value={busca}
                onChange={(e) => setBusca(e.target.value)}
                onKeyDown={onKeyDown}
                placeholder={searchPlaceholder}
                aria-label={searchPlaceholder}
                aria-controls={listaId}
                className="field !py-1.5 !pl-7 !pr-2 text-xs"
              />
            </div>
          )}

          {visiveis.length === 0 && (
            <p className="text-text-muted text-xs px-2.5 py-3 text-center">
              Nada encontrado para “{busca.trim()}”.
            </p>
          )}

          <ul
            ref={listaRef}
            id={listaId}
            role="listbox"
            aria-label="Opções"
            className="overflow-y-auto"
            style={{ maxHeight: Math.min(posicao.maxHeight, 320) }}
          >
            {visiveis.map((o, i) => {
            const marcado = o.value === value;
            return (
              <li
                key={String(o.value)}
                id={`${listaId}-${i}`}
                role="option"
                aria-selected={marcado}
                aria-disabled={o.disabled || undefined}
                onPointerEnter={() => !o.disabled && setAtivo(i)}
                onClick={() => escolher(i)}
                className={[
                  "select-opcao",
                  o.disabled ? "opacity-50 cursor-not-allowed" : "",
                  i === ativo && !o.disabled ? "is-ativa" : "",
                  marcado ? "is-marcada" : "",
                ].join(" ")}
              >
                <Check
                  aria-hidden="true"
                  className={[
                    "w-3.5 h-3.5 mt-0.5 shrink-0 text-selected",
                    marcado ? "" : "invisible",
                  ].join(" ")}
                />
                <span className="min-w-0">
                  <span className="flex items-center gap-1.5 text-xs font-medium leading-snug">
                    {o.icon}
                    {o.label}
                  </span>
                  {o.hint && (
                    <span className="block text-text-muted text-[10px] leading-snug">{o.hint}</span>
                  )}
                  </span>
                </li>
              );
            })}
          </ul>
        </div>,
        document.body
      )}
    </div>
  );
}
