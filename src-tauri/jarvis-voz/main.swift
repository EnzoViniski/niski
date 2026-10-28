// Voz do Jarvis (macOS 26+). Processo auxiliar que ouve com o reconhecimento de fala da Apple
// (SpeechAnalyzer, no próprio Mac) e fala com a voz do sistema (AVSpeechSynthesizer). Áudio e texto
// não vão para disco nem para a rede. Conversa com o app por JSON, uma linha por mensagem:
//   entrada (stdin):  {"cmd":"ouvir"} {"cmd":"pausar"} {"cmd":"falar","id":1,"texto":"..."}
//                     {"cmd":"calar"} {"cmd":"voz","id":"com.apple..."} {"cmd":"interromper","ligado":true}
//                     {"cmd":"sair"}
//   saída (stdout):   pronto, parcial, final, nivel, falando, falou, calado, interrompido, eco, erro
// A fala sai pelo mesmo AVAudioEngine que ouve, com o processamento de voz ligado: o macOS tira a
// voz do próprio Jarvis do microfone (cancelamento de eco), e o Enzo consegue interromper falando.
// Compilado por `scripts/build-voz-do-jarvis.mjs`. `--arquivo <audio>` transcreve um arquivo (teste).
// `--vozes` lista só metadados; `--verificar` testa a política de voz, sem abrir áudio.
import AVFoundation
import Foundation
import Speech

let filaDeSaida = DispatchQueue(label: "jarvis-voz.saida")
func emitir(_ objeto: [String: Any]) {
    guard let dados = try? JSONSerialization.data(withJSONObject: objeto) else { return }
    filaDeSaida.async {
        FileHandle.standardOutput.write(dados)
        FileHandle.standardOutput.write(Data([0x0A]))
    }
}

func palavras(_ texto: String) -> [String] {
    texto.lowercased().folding(options: .diacriticInsensitive, locale: nil)
        .components(separatedBy: CharacterSet.alphanumerics.inverted).filter { $0.count > 1 }
}

/// Parte do que se ouviu enquanto o Jarvis falava que é a própria fala dele (eco que escapou do
/// cancelamento). Acima de 60% das palavras repetidas, descarta.
func pareceEco(_ ouvido: String, falado: String) -> Bool {
    let ditas = Set(palavras(falado))
    let ouvidas = palavras(ouvido)
    guard !ouvidas.isEmpty, !ditas.isEmpty else { return false }
    let repetidas = ouvidas.filter { ditas.contains($0) }.count
    return Double(repetidas) / Double(ouvidas.count) >= 0.6
}

func rms(_ buffer: AVAudioPCMBuffer) -> Float {
    guard let canal = buffer.floatChannelData?[0], buffer.frameLength > 0 else { return 0 }
    var soma: Float = 0
    for i in 0..<Int(buffer.frameLength) { soma += canal[i] * canal[i] }
    let valor = (soma / Float(buffer.frameLength)).squareRoot()
    // dB de -60 a 0 → 0 a 1, como o `voz_niveis` do chat.
    let db = 20 * log10(max(valor, 1e-6))
    return max(0, min(1, (db + 60) / 60))
}

/// Converte qualquer buffer para `destino` (taxa e formato), reaproveitando o conversor.
final class Conversor {
    let destino: AVAudioFormat
    private var conversor: AVAudioConverter?
    init(destino: AVAudioFormat) { self.destino = destino }

    func converter(_ buffer: AVAudioPCMBuffer) -> AVAudioPCMBuffer? {
        if buffer.format == destino { return buffer }
        if conversor?.inputFormat != buffer.format {
            conversor = AVAudioConverter(from: buffer.format, to: destino)
        }
        guard let conversor else { return nil }
        let capacidade = AVAudioFrameCount(Double(buffer.frameLength) * destino.sampleRate / buffer.format.sampleRate) + 32
        guard let saida = AVAudioPCMBuffer(pcmFormat: destino, frameCapacity: capacidade) else { return nil }
        var entregue = false
        var erro: NSError?
        conversor.convert(to: saida, error: &erro) { _, estado in
            if entregue { estado.pointee = .noDataNow; return nil }
            entregue = true
            estado.pointee = .haveData
            return buffer
        }
        return erro == nil ? saida : nil
    }
}

/// Só o canal 0, em float: com o processamento de voz o macOS pode entregar vários canais.
func mono(_ buffer: AVAudioPCMBuffer) -> AVAudioPCMBuffer? {
    let formato = buffer.format
    // Copia também mono: o tap de áudio reutiliza o buffer antes de a fila principal consumi-lo.
    guard formato.commonFormat == .pcmFormatFloat32, let origem = buffer.floatChannelData?[0],
          let alvo = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: formato.sampleRate, channels: 1, interleaved: false),
          let saida = AVAudioPCMBuffer(pcmFormat: alvo, frameCapacity: buffer.frameLength),
          let destino = saida.floatChannelData?[0] else { return nil }
    destino.update(from: origem, count: Int(buffer.frameLength))
    saida.frameLength = buffer.frameLength
    return saida
}

final class Voz: @unchecked Sendable {
    let motor = AVAudioEngine()
    let tocador = AVAudioPlayerNode()
    let sintetizador = AVSpeechSynthesizer()
    let formatoDaFala = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 48_000, channels: 1, interleaved: false)!
    lazy var conversorDaFala = Conversor(destino: formatoDaFala)
    var conversorDoMic: Conversor?
    var entrada: AsyncStream<AnalyzerInput>.Continuation?
    var analisador: SpeechAnalyzer?
    var transcritor: SpeechTranscriber?

    // Estado tocado só na fila principal.
    var ouvindo = false
    var interromperComVoz = true
    var ecoDisponivel = false
    var voz: AVSpeechSynthesisVoice?
    var filaDeFalas: [(id: Int, texto: String)] = []
    var falandoAgora: (id: Int, texto: String)?
    var geracao = 0 // sobe a cada "calar": buffers atrasados de uma fala cancelada são ignorados
    var agendados = 0
    var sinteseTerminou = false
    var recebeuAudioDaSintese = false
    var falouHaPouco = ""
    var nivelDoMic: Float = 0
    var nivelDaFala: Float = 0
    let relogio = DispatchSource.makeTimerSource(queue: .main)

    func preparar() throws {
        // A saída tem que existir antes de ligar o processamento de voz; ao contrário, o macOS deixa
        // o nó de saída sem canais e o `start` falha (-10875).
        motor.attach(tocador)
        motor.connect(tocador, to: motor.mainMixerNode, format: formatoDaFala)
        _ = motor.outputNode
        let entradaDoMic = motor.inputNode
        do {
            try entradaDoMic.setVoiceProcessingEnabled(true)
            ecoDisponivel = true
            entradaDoMic.voiceProcessingOtherAudioDuckingConfiguration =
                AVAudioVoiceProcessingOtherAudioDuckingConfiguration(enableAdvancedDucking: false, duckingLevel: .min)
        } catch {
            emitir(["ev": "aviso", "msg": "Sem cancelamento de eco neste Mac; interromper falando fica desligado."])
            interromperComVoz = false
        }
        motor.mainMixerNode.installTap(onBus: 0, bufferSize: 1024, format: nil) { [weak self] buffer, _ in
            let nivel = rms(buffer)
            DispatchQueue.main.async { self?.nivelDaFala = nivel }
        }
        let formatoDoMic = entradaDoMic.outputFormat(forBus: 0)
        entradaDoMic.installTap(onBus: 0, bufferSize: 1024, format: formatoDoMic) { [weak self] buffer, _ in
            self?.chegouDoMic(buffer)
        }
        motor.prepare()
        try motor.start()
        if voz == nil { voz = VozesDoJarvis.padrao() }
        // `dispatchMain()` não roda RunLoop, então nada de `Timer`: relógio do próprio dispatch.
        relogio.schedule(deadline: .now(), repeating: .milliseconds(50))
        relogio.setEventHandler { [weak self] in
            guard let self else { return }
            if self.falandoAgora == nil { self.nivelDaFala = 0 }
            emitir(["ev": "nivel", "mic": self.ouvindo ? self.nivelDoMic : 0, "fala": self.nivelDaFala])
        }
        relogio.resume()
    }

    func chegouDoMic(_ buffer: AVAudioPCMBuffer) {
        guard let canal = mono(buffer) else { return }
        let nivel = rms(canal)
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.nivelDoMic = nivel
            guard self.ouvindo, let conversor = self.conversorDoMic, let pronto = conversor.converter(canal) else { return }
            self.entrada?.yield(AnalyzerInput(buffer: pronto))
        }
    }

    func iniciarTranscricao() async throws {
        let pedida = Locale(identifier: "pt-BR")
        let idioma = await SpeechTranscriber.supportedLocale(equivalentTo: pedida) ?? pedida
        let transcritor = SpeechTranscriber(locale: idioma, transcriptionOptions: [],
                                            reportingOptions: [.volatileResults, .fastResults], attributeOptions: [])
        if !(await SpeechTranscriber.installedLocales.contains { $0.identifier(.bcp47) == idioma.identifier(.bcp47) }),
           let pedido = try await AssetInventory.assetInstallationRequest(supporting: [transcritor]) {
            emitir(["ev": "baixando_modelo"])
            try await pedido.downloadAndInstall()
        }
        let analisador = SpeechAnalyzer(modules: [transcritor])
        guard let formato = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [transcritor]) else {
            throw NSError(domain: "jarvis-voz", code: 1, userInfo: [NSLocalizedDescriptionKey: "Sem formato de áudio para o reconhecimento"])
        }
        let (fluxo, continuacao) = AsyncStream<AnalyzerInput>.makeStream()
        try await analisador.start(inputSequence: fluxo)
        await MainActor.run {
            self.conversorDoMic = Conversor(destino: formato)
            self.entrada = continuacao
            self.analisador = analisador
            self.transcritor = transcritor
        }
        Task { [weak self] in
            do {
                for try await resultado in transcritor.results {
                    let texto = String(resultado.text.characters).trimmingCharacters(in: .whitespacesAndNewlines)
                    guard !texto.isEmpty else { continue }
                    let final = resultado.isFinal
                    await MainActor.run { [weak self] in self?.ouviu(texto, final: final) }
                }
            } catch {
                emitir(["ev": "erro", "msg": "O reconhecimento de fala parou: \(error.localizedDescription)"])
            }
        }
    }

    func ouviu(_ texto: String, final: Bool) {
        // Resultados já enfileirados podem chegar depois do botão Pausar.
        guard ouvindo else { return }
        let falando = falandoAgora != nil
        let referencia = (falandoAgora?.texto ?? "") + " " + falouHaPouco
        let parar = pedeParaCalar(texto)
        // Logo depois de falar, uma palavra solta é quase sempre a cauda do eco ("Teste" → "Tempo").
        let cauda = !falouHaPouco.isEmpty && palavras(texto).count < 2
        if !parar && (cauda || ((falando || !falouHaPouco.isEmpty) && pareceEco(texto, falado: referencia))) {
            if final { emitir(["ev": "eco"]) }
            return
        }
        if falando {
            // Duas palavras de verdade por cima da fala = o Enzo interrompeu.
            guard ecoDisponivel, interromperComVoz, parar || palavras(texto).count >= 2 else { return }
            calar()
            emitir(["ev": "interrompido"])
        }
        emitir(["ev": final ? "final" : "parcial", "texto": texto])
    }

    // MARK: fala

    func emitirVozes(_ evento: String = "vozes") {
        var catalogo = VozesDoJarvis.catalogo(escolhida: voz)
        catalogo["ev"] = evento
        if evento == "pronto" { catalogo["eco"] = ecoDisponivel }
        emitir(catalogo)
    }

    func escolherVoz(_ id: String) {
        // Só aceita o catálogo local; identificador antigo/removido não cai silenciosamente na voz padrão.
        guard let escolhida = id.isEmpty ? VozesDoJarvis.padrao() : VozesDoJarvis.disponiveis().first(where: { $0.identifier == id }) else {
            emitir(["ev": "erro", "codigo": "voz_indisponivel", "msg": "Esta voz não está disponível no Mac. Escolha uma das vozes instaladas em português."])
            return
        }
        voz = escolhida
        emitirVozes()
    }

    func falar(id: Int, texto: String) {
        guard !texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            emitir(["ev": "falou", "id": id]); return
        }
        guard voz != nil else {
            emitir(["ev": "erro", "codigo": "sem_voz", "msg": "Nenhuma voz em português está instalada. Adicione uma voz em Ajustes do Sistema → Acessibilidade → Leitura e Fala → informações ao lado de Voz do sistema."])
            emitir(["ev": "falou", "id": id]); return
        }
        filaDeFalas.append((id, texto))
        if falandoAgora == nil { proximaFala() }
    }

    func proximaFala() {
        guard falandoAgora == nil, !filaDeFalas.isEmpty else { return }
        let fala = filaDeFalas.removeFirst()
        falandoAgora = fala
        agendados = 0
        sinteseTerminou = false
        recebeuAudioDaSintese = false
        let minhaGeracao = geracao
        let frase = AVSpeechUtterance(string: fala.texto)
        frase.voice = voz
        // Prosódia próxima da voz original; baixar demais o tom cria um efeito robótico.
        frase.rate = AVSpeechUtteranceDefaultSpeechRate * 0.96
        frase.pitchMultiplier = 1.0
        frase.postUtteranceDelay = 0.08
        if !motor.isRunning {
            do { try motor.start() } catch {
                calar()
                emitir(["ev": "erro", "codigo": "saida_audio", "msg": "A saída de áudio do Jarvis parou. Confira o dispositivo de som e reabra o Jarvis."])
                return
            }
        }
        emitir(["ev": "falando", "id": fala.id])
        sintetizador.write(frase) { [weak self] buffer in
            DispatchQueue.main.async {
                guard let self, self.geracao == minhaGeracao else { return }
                guard let pcm = buffer as? AVAudioPCMBuffer, pcm.frameLength > 0 else {
                    self.sinteseTerminou = true
                    self.talvezTerminou(minhaGeracao)
                    return
                }
                guard let pronto = self.conversorDaFala.converter(pcm) else {
                    self.calar()
                    emitir(["ev": "erro", "codigo": "formato_audio", "msg": "Não foi possível reproduzir esta voz. Escolha outra voz instalada."])
                    return
                }
                self.agendados += 1
                self.recebeuAudioDaSintese = true
                // Espera o áudio sair de fato do alto-falante, inclusive a latência do dispositivo.
                // O callback padrão (.dataConsumed) terminava cedo e desarmava o filtro de eco.
                self.tocador.scheduleBuffer(pronto, completionCallbackType: .dataPlayedBack) { _ in
                    DispatchQueue.main.async {
                        guard self.geracao == minhaGeracao else { return }
                        self.agendados -= 1
                        self.talvezTerminou(minhaGeracao)
                    }
                }
                if !self.tocador.isPlaying { self.tocador.play() }
            }
        }
    }

    func talvezTerminou(_ minhaGeracao: Int) {
        guard geracao == minhaGeracao, sinteseTerminou, agendados == 0, let fala = falandoAgora else { return }
        if !recebeuAudioDaSintese {
            emitir(["ev": "erro", "codigo": "voz_sem_audio", "msg": "O macOS não gerou áudio para esta voz. Escolha outra voz instalada ou conclua o download nos ajustes de acessibilidade."])
        }
        falandoAgora = nil
        agendados = 0
        sinteseTerminou = false
        nivelDaFala = 0
        lembrarDoQueFalou(fala.texto)
        emitir(["ev": "falou", "id": fala.id])
        proximaFala()
    }

    /// O reconhecimento ainda entrega o fim da última frase um instante depois: guarda o texto por
    /// 1,5 s para o filtro de eco.
    func lembrarDoQueFalou(_ texto: String) {
        falouHaPouco = texto
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { [weak self] in
            if self?.falouHaPouco == texto { self?.falouHaPouco = "" }
        }
    }

    func calar() {
        geracao += 1
        filaDeFalas.removeAll()
        if let fala = falandoAgora { lembrarDoQueFalou(fala.texto) }
        falandoAgora = nil
        agendados = 0
        sinteseTerminou = false
        nivelDaFala = 0
        sintetizador.stopSpeaking(at: .immediate)
        tocador.stop()
        emitir(["ev": "calado"])
    }

    func encerrar() {
        calar()
        entrada?.finish()
        motor.stop()
        if let analisador { Task { await analisador.cancelAndFinishNow() } }
    }
}

func pedirPermissoes() async -> Bool {
    switch AVCaptureDevice.authorizationStatus(for: .audio) {
    case .authorized: return true
    case .notDetermined: return await AVCaptureDevice.requestAccess(for: .audio)
    default: return false
    }
}

func transcreverArquivo(_ caminho: String) async throws {
    let transcritor = SpeechTranscriber(locale: Locale(identifier: "pt-BR"), transcriptionOptions: [],
                                        reportingOptions: [.volatileResults, .fastResults], attributeOptions: [])
    let analisador = SpeechAnalyzer(modules: [transcritor])
    let arquivo = try AVAudioFile(forReading: URL(fileURLWithPath: caminho))
    let inicio = Date()
    async let leitura: Void = {
        for try await resultado in transcritor.results where resultado.isFinal {
            emitir(["ev": "final", "texto": String(resultado.text.characters), "s": Date().timeIntervalSince(inicio)])
        }
    }()
    if let fim = try await analisador.analyzeSequence(from: arquivo) {
        try await analisador.finalizeAndFinish(through: fim)
    } else {
        try await analisador.finalizeAndFinishThroughEndOfInput()
    }
    try await leitura
}

let argumentos = CommandLine.arguments
if argumentos.contains("--vozes") {
    emitir(VozesDoJarvis.catalogo(escolhida: VozesDoJarvis.padrao()))
    filaDeSaida.sync {}
    exit(0)
}
if argumentos.contains("--verificar") {
    let sucesso = verificarPoliticaDeVoz()
        && pareceEco("Uma resposta do Jarvis", falado: "Esta é uma resposta do Jarvis.")
        && !pareceEco("Quero mudar o projeto", falado: "Esta é uma resposta do Jarvis.")
    emitir(["ev": "verificacao", "ok": sucesso])
    filaDeSaida.sync {}
    exit(sucesso ? 0 : 1)
}
if let i = argumentos.firstIndex(of: "--arquivo"), i + 1 < argumentos.count {
    let caminho = argumentos[i + 1]
    Task {
        do { try await transcreverArquivo(caminho) } catch { emitir(["ev": "erro", "msg": error.localizedDescription]) }
        filaDeSaida.sync {}
        exit(0)
    }
    dispatchMain()
}

let voz = Voz()

func tratar(_ linha: String) {
    guard let dados = linha.data(using: .utf8),
          let comando = (try? JSONSerialization.jsonObject(with: dados)) as? [String: Any],
          let nome = comando["cmd"] as? String else { return }
    switch nome {
    case "ouvir": voz.ouvindo = true
    case "pausar": voz.ouvindo = false; voz.nivelDoMic = 0
    case "falar":
        if let texto = comando["texto"] as? String, let id = comando["id"] as? Int { voz.falar(id: id, texto: texto) }
    case "calar": voz.calar()
    case "interromper": voz.interromperComVoz = (comando["ligado"] as? Bool) ?? true
    case "voz":
        if let id = comando["id"] as? String { voz.escolherVoz(id) }
    case "vozes": voz.emitirVozes()
    case "sair":
        voz.encerrar()
        filaDeSaida.sync {}
        exit(0)
    default: break
    }
}

Task { @MainActor in
    guard await pedirPermissoes() else {
        emitir(["ev": "erro", "codigo": "microfone", "msg": "Libere o Niski em Ajustes do Sistema → Privacidade e Segurança → Microfone."])
        return
    }
    do {
        try voz.preparar()
        try await voz.iniciarTranscricao()
        voz.emitirVozes("pronto")
        if voz.voz?.gender != .male {
            emitir(["ev": "aviso", "msg": "Não há voz masculina natural em português disponível neste Mac. Adicione uma voz masculina aprimorada ou premium em Ajustes do Sistema → Acessibilidade → Leitura e Fala → informações ao lado de Voz do sistema e escolha no Jarvis. Até lá, está em uso a voz disponível."])
        } else if voz.voz?.quality == .default {
            emitir(["ev": "aviso", "msg": "A voz masculina disponível usa qualidade padrão. Você pode adicionar uma voz aprimorada ou premium nos ajustes de acessibilidade do macOS e escolher no Jarvis."])
        }
    } catch {
        emitir(["ev": "erro", "msg": "A voz não abriu: \(error.localizedDescription)"])
    }
}

// Comandos chegam por linha; o fim da entrada (app fechou) encerra o processo.
Thread.detachNewThread {
    while let linha = readLine() {
        DispatchQueue.main.async { tratar(linha) }
    }
    DispatchQueue.main.async { voz.encerrar(); filaDeSaida.sync {}; exit(0) }
}

dispatchMain()
