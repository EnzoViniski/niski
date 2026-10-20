import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import "./index.css";
import "./styles/fork.css";
// Fork: pausa de animações com a janela parada e contenção de layout (energia).
import "./styles/energia.css";
import { usarTransporte } from "./mobile/api";
import { transporteDoEngine } from "./features/conversa/transporteDoEngine";
import { instalarRegistroDeErros } from "./lib/registroDeErros";
// Fork: aparência/workspaces também em disco; ao abrir, restaura de lá se a cópia for mais nova.
import { iniciarEspelho, restaurarDoDisco } from "./lib/espelhoDaInterface";

instalarRegistroDeErros();

// A visão Conversa das abas de agente usa o chat da web; aqui ele fala com o engine pelo protocolo
// local em vez de HTTP.
usarTransporte(transporteDoEngine);

const noTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

void (noTauri ? restaurarDoDisco() : Promise.resolve("sem-copia")).then(() => {
  ReactDOM.createRoot(document.getElementById("root")!).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>
  );
  if (noTauri) iniciarEspelho();
});
