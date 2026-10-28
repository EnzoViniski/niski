import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import "./index.css";
import "./styles/niski.css";
// Pausa de animações com a janela parada e contenção de layout (energia).
import "./styles/energia.css";
// Estilo Minimalista (só vale com data-estilo="minimalista"; por último para reescrever os outros).
import "./styles/minimalista.css";
// Estilo Vidro (só vale com data-variante="vidro"; depois do Minimalista, que ele reaproveita).
import "./styles/vidro.css";
// Símbolo do Niski conforme o tema.
import "./styles/marca.css";
import { usarTransporte } from "./mobile/api";
import { transporteDoEngine } from "./features/conversa/transporteDoEngine";
import { instalarRegistroDeErros } from "./lib/registroDeErros";
// Aparência/workspaces também em disco; ao abrir, restaura de lá se a cópia for mais nova.
import { iniciarEspelho, restaurarDoDisco } from "./lib/espelhoDaInterface";

instalarRegistroDeErros();

// A visão Conversa das abas de agente usa o chat da web; aqui ele fala com o engine pelo protocolo
// local em vez de HTTP.
usarTransporte(transporteDoEngine);

const noTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
// A janela flutuante do Jarvis carrega este mesmo index.html. Ela mostra só o Jarvis e não toca
// no estado da interface (nada de restaurar nem espelhar workspaces/aparência).
const ehJarvis = noTauri && (window as { __TAURI_INTERNALS__?: { metadata?: { currentWindow?: { label?: string } } } })
  .__TAURI_INTERNALS__?.metadata?.currentWindow?.label === "jarvis";

if (ehJarvis) {
  document.documentElement.classList.add("janela-jarvis");
  void import("./features/jarvis/JanelaDoJarvis").then(({ JanelaDoJarvis }) => {
    ReactDOM.createRoot(document.getElementById("root")!).render(<JanelaDoJarvis />);
  });
} else void (noTauri ? restaurarDoDisco() : Promise.resolve("sem-copia")).then(() => {
  ReactDOM.createRoot(document.getElementById("root")!).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>
  );
  if (noTauri) iniciarEspelho();
});
