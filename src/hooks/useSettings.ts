import { useState, useCallback, useEffect } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { AppSettings } from "../types/settings";
import { DEFAULT_SETTINGS } from "../types/settings";
import { loadSettings, saveSettings } from "../services/settingsService";
import { setMotionLevel } from "../lib/motion";
import { setShortcutOverrides } from "../lib/shortcuts";
import { aplicarPaleta } from "../lib/aplicarTema";
import { aplicarEstilo } from "../lib/estiloDaInterface"; // Fork: Clássico, Minimalista ou Vidro.
import { isBackgroundEffect } from "../components/backgrounds/registry";

export function useSettings() {
  const [settings, setSettings] = useState<AppSettings>(() => loadSettings());

  // Fora de effect de propósito: os hooks de GSAP dos filhos rodam logo após
  // este render e precisam do nível já atualizado.
  setMotionLevel(settings.animations);
  setShortcutOverrides(settings.shortcuts);

  const updateSettings = useCallback((updates: Partial<AppSettings>) => {
    setSettings((prev) => {
      const next = { ...prev, ...updates };
      saveSettings(next);
      return next;
    });
  }, []);

  const resetSettings = useCallback(() => {
    setSettings({ ...DEFAULT_SETTINGS });
    saveSettings({ ...DEFAULT_SETTINGS });
  }, []);

  // Tema + paleta de cor. Andam juntos de propósito: a paleta tem variante
  // para o tema claro, então quem resolve "sistema → claro/escuro" é também
  // quem sabe qual variante escrever. Separar em dois efeitos deixaria a cor
  // um render atrás do tema ao seguir o Windows.
  useEffect(() => {
    const root = document.documentElement;

    const aplicar = (claro: boolean) => {
      aplicarPaleta(root, settings.accent, claro);
      // Fork: o ícone da janela fica o do app (Niski); o upstream repintava a logo do OMNI aqui.
    };

    if (settings.theme !== "sistema") {
      aplicar(settings.theme === "claro");
      return;
    }
    const media = window.matchMedia?.("(prefers-color-scheme: light)");
    const seguirSistema = () => aplicar(!!media?.matches);
    seguirSistema();
    media?.addEventListener("change", seguirSistema);
    return () => media?.removeEventListener("change", seguirSistema);
  }, [settings.theme, settings.accent]);

  // Aparência: fundo, intensidade do vidro e movimento (ver index.css).
  useEffect(() => {
    const root = document.documentElement;
    // Efeitos animados são desenhados pelo <AppBackground>, não pelo CSS —
    // um data-bg genérico desliga as camadas do body (ver index.css).
    root.dataset.bg = isBackgroundEffect(settings.background) ? "efeito" : settings.background;
    root.dataset.glass = settings.glass;
    root.dataset.motion = settings.animations;
    root.dataset.conversa = settings.conversationBackground;
    // Fork: estilo da interface. Valor desconhecido (de uma versão mais nova) fica no clássico.
    // Fork: o Vidro liga o Minimalista mais a variante (`estiloDaInterface.ts`).
    aplicarEstilo(root, settings.estilo);
    // 100% vira alfa .25 da cor do tema: acima disso a linha disputa com o texto.
    root.style.setProperty("--conversa-grade", String(settings.conversationGridOpacity / 400));
    root.style.setProperty("--app-bg-opacity", String(settings.backgroundOpacity / 100));
    root.style.setProperty("--app-bg-blur", `${settings.backgroundBlur}px`);
    root.style.setProperty("--terminal-opacity", String(settings.terminalOpacity / 100));

    if (settings.background === "custom" && settings.backgroundPath) {
      // Aspas simples na url() evitam quebrar o valor com caminhos que tenham ".
      const src = convertFileSrc(settings.backgroundPath).replace(/'/g, "%27");
      root.style.setProperty("--app-bg-image", `url('${src}')`);
    } else {
      // Deixa o preset de [data-bg] no index.css valer.
      root.style.removeProperty("--app-bg-image");
    }
  }, [
    settings.background,
    settings.backgroundPath,
    settings.backgroundOpacity,
    settings.backgroundBlur,
    settings.terminalOpacity,
    settings.glass,
    settings.animations,
    settings.conversationBackground,
    settings.conversationGridOpacity,
    settings.estilo,
  ]);

  return { settings, updateSettings, resetSettings };
}
