use crate::{
    i18n::{pair, t},
    theme::*,
};
use eframe::egui::{self, RichText};
use rf_core::Kind;
pub struct Doc {
    pub purpose: String,
    pub formula: &'static str,
    pub example: &'static str,
    pub limits: &'static str,
}
pub fn document(kind: Kind) -> Doc {
    if let Kind::Dsp(op) = kind {
        return dsp_document(op);
    }
    let (purpose, formula, example, limits) = match kind {
        Kind::Dsp(op) => (
            op.label(),
            "Voir paramètres typés et guide RF/DSP 0.4",
            "Projet → Démo DSP / Modem",
            "Profil de recherche ; paramètres, références et unités explicites. Validation matérielle requise.",
        ),
        Kind::Generator => (
            pair(
                "Source RF réglable en fréquence et puissance. Sa sortie transporte un niveau RF, pas une forme d'onde temporelle.",
                "RF source with configurable frequency and power. Its output carries an RF level, not time-domain samples.",
            ),
            "P[mW] = 10^(P[dBm]/10)",
            pair(
                "Générateur → DUT → analyseur ; 2,45 GHz, −10 dBm.",
                "Generator → DUT → analyzer; 2.45 GHz, −10 dBm.",
            ),
            pair(
                "SCPI générique existant ; vérifier le profil du modèle avant le matériel.",
                "Existing generic SCPI profile; validate it against the instrument manual.",
            ),
        ),
        Kind::Analyzer => (
            pair(
                "Produit une trace amplitude/fréquence en dBm depuis l'entrée RF.",
                "Produces an amplitude/frequency trace in dBm from its RF input.",
            ),
            "P_peak = max(amplitude_dbm)",
            pair(
                "Spectre 2,40–2,50 GHz, 401 points ; envoyer TRACE vers Python ou Détection de pic.",
                "Sweep 2.40–2.50 GHz, 401 points; connect TRACE to Python or Peak detector.",
            ),
            pair(
                "La simulation illustre porteuse et bruit. RBW, VBW et incertitude ne sont pas modélisées.",
                "Simulation illustrates a carrier and noise. RBW, VBW and uncertainty are not modeled.",
            ),
        ),
        Kind::Dut => (
            pair(
                "Modèle du dispositif sous test ; perte RF et facteur de bruit, avec une fiche facultative du catalogue.",
                "Device under test model; RF loss and noise figure, optionally linked to a catalogue entry.",
            ),
            "P_out[dBm] = P_in[dBm] − loss[dB]",
            pair(
                "SIGNAL IN reçoit les données du générateur. MODEL alimente le PNA ou NF Meter ; les broches rondes RF décrivent les liaisons physiques.",
                "SIGNAL IN receives generator data. MODEL feeds the PNA or NF meter; round RF pins describe physical links.",
            ),
            pair(
                "Fiches MACOM sourcées et modèle linéaire de référence ; pas de compression ni solveur de circuit. Les câbles ne commandent pas de commutateur RF.",
                "Catalogue snapshot; no manufacturer model or automatic coaxial routing.",
            ),
        ),
        Kind::Pna | Kind::PnaX | Kind::UsbVna => (
            pair(
                "Balayage réseau avec magnitude et phase. Choisir S11/S22 pour le Smith, S21/S12 pour la transmission.",
                "Network sweep with magnitude and phase. Select S11/S22 for Smith or S21/S12 for transmission.",
            ),
            "Γ = 10^(magnitude_dB/20) · exp(j·phase_deg·π/180)\nZ = Z₀ · (1+Γ)/(1−Γ)",
            pair(
                "DUT MODEL → PNA ; régler S11, Exécuter, ouvrir Smith.",
                "DUT MODEL → PNA; choose S11, Run, open Smith.",
            ),
            pair(
                "Simulation idéale ; acquisition SDATA binaire sur PNA/PNA-X Keysight et VNA USB Keysight P50xx/P93xx identifié. Aucune calibration SOLT/TRL automatique.",
                "Ideal simulation; binary SDATA on identified Keysight PNA/PNA-X and P50xx/P93xx USB VNA. No automatic SOLT/TRL calibration.",
            ),
        ),
        Kind::DcSupplyE3631A | Kind::DcSupplyE36313A => (
            pair(
                "Alimentation DC à trois sorties : V, limite I, lecture et ON/OFF explicites.",
                "Three-output DC supply: V, current limit, readback and explicit ON/OFF.",
            ),
            "P[W] = V[V] × I[A]",
            pair(
                "Configurer le bloc ; Appliquer (OFF), puis ON ; Stop coupe les sorties activées.",
                "Configure; Apply (OFF), then ON; Stop disables outputs enabled by this session.",
            ),
            pair(
                "E3631A : ON/OFF global. E36313A : mode indépendant FIX. Pas de solveur de charge DC ni de séquence automatique de polarisation.",
                "E3631A: global ON/OFF. E36313A: independent FIX mode. No DC load solver or automatic bias sequence.",
            ),
        ),
        Kind::Awg => (
            pair(
                "Génère deux séquences numériques I et Q normalisées, en quadrature.",
                "Generates two normalized digital I and Q sequences in quadrature.",
            ),
            "I[n] = sin(2π f₀ n / f_s)\nQ[n] = cos(2π f₀ n / f_s)",
            pair(
                "AWG I/Q → deux DAC → modulateur I/Q ; 100 MS/s, tonalité 1 MHz.",
                "AWG I/Q → two DACs → I/Q modulator; 100 MS/s, 1 MHz tone.",
            ),
            pair(
                "Tonalité idéale quantifiée ; aucun générateur QAM/OFDM. La constellation montre une trajectoire circulaire.",
                "Ideal quantized tone; no QAM/OFDM generator. Constellation shows a circular trajectory.",
            ),
        ),
        Kind::Dac => (
            pair(
                "Convertit des échantillons numériques normalisés en volts.",
                "Converts normalized digital samples into volts.",
            ),
            "V_out = quantize(x, bits) · V_FS",
            pair(
                "AWG I → DAC DATA ; sortie V vers l'entrée I du modulateur.",
                "AWG I → DAC DATA; voltage output to modulator I input.",
            ),
            pair(
                "Cadence d'entrée conservée ; pas de rééchantillonnage, filtre de reconstruction, jitter ou non-linéarité.",
                "Input rate is preserved; no resampling, reconstruction filter, jitter or nonlinearity.",
            ),
        ),
        Kind::Adc => (
            pair(
                "Échantillons analogiques en volts vers valeurs numériques normalisées.",
                "Analog voltage samples to normalized digital values.",
            ),
            "x = quantize(clamp(V_in/V_FS, −1, +1), bits)",
            pair(
                "DAC OUT → CAN IN ; inspecter le buffer DATA pour l'écrêtage.",
                "DAC OUT → ADC IN; inspect DATA for clipping.",
            ),
            pair(
                "Conversion idéale à cadence conservée ; pas de modèle d'ouverture ou bruit de quantification aléatoire.",
                "Ideal conversion at the input rate; no aperture model or random quantization noise.",
            ),
        ),
        Kind::IqModulator => (
            pair(
                "Associe deux buffers analogiques I/Q et une source LO RF.",
                "Combines two analog I/Q buffers and an RF LO source.",
            ),
            "f_RF = f_LO + f_base\nP_RF = P_LO + 10·log₁₀(mean(I²+Q²)) − loss",
            pair(
                "Deux DAC vers I et Q ; générateur vers LO ; RF OUT vers analyseur.",
                "Two DACs to I and Q; generator to LO; RF OUT to analyzer.",
            ),
            pair(
                "Cadences, unités et longueurs cohérentes ; modèle idéal sans imbalance ni images spectrales.",
                "Consistent rates, units and lengths required; ideal model without imbalance or spectral images.",
            ),
        ),
        Kind::VariableResistor => (
            pair(
                "Valeur de résistance réglable, transmise avec son unité.",
                "Adjustable resistance value with explicit units.",
            ),
            "R_out[Ω] = R_configured",
            pair(
                "Régler 50 Ω ; retrouver la valeur dans Mesures ou une sonde.",
                "Set 50 Ω; inspect the value in Measurements or a probe.",
            ),
            pair(
                "Aucun solveur de circuit électrique. La formule V=RI n'est pas exécutée par ce bloc.",
                "No circuit solver. This block does not evaluate V=RI.",
            ),
        ),
        Kind::Thermometer => (
            pair(
                "Lit une température en entrée, ou sa valeur locale si l'entrée est libre.",
                "Reads input temperature or a local value when unconnected.",
            ),
            "T_out[°C] = T_in or T_configured",
            pair(
                "Thermostream TEMP → thermomètre TEMP.",
                "Thermostream TEMP → thermometer TEMP.",
            ),
            pair(
                "Pas d'étalonnage, d'incertitude ou de retard de capteur.",
                "No calibration, uncertainty or sensor delay.",
            ),
        ),
        Kind::Thermostream => (
            pair(
                "Consigne thermique simulée, utilisable comme source TEMP.",
                "Simulated thermal setpoint exposed as a TEMP source.",
            ),
            "T_out[°C] = T_setpoint",
            pair(
                "Régler 85 °C, connecter un thermomètre, Exécuter.",
                "Set 85 °C, connect a thermometer, Run.",
            ),
            pair(
                "Aucune dynamique thermique ni commande de soufflerie physique.",
                "No thermal dynamics or physical airflow control.",
            ),
        ),
        Kind::NoiseFigureMeter => (
            pair(
                "Affiche le facteur de bruit du modèle DUT, ou une valeur locale.",
                "Displays DUT model noise figure or a local value.",
            ),
            "NF[dB] = NF_DUT or NF_configured\nDefinition: F = SNR_in / SNR_out ; NF = 10·log₁₀(F)",
            pair(
                "DUT MODEL → NF Meter ; NF 2,5 dB dans la démonstration.",
                "DUT MODEL → NF meter; demo NF is 2.5 dB.",
            ),
            pair(
                "La définition explique l'unité ; aucune mesure Y-factor, source ENR ou correction de pertes.",
                "The definition explains the units; no Y-factor, ENR source or loss correction measurement.",
            ),
        ),
        Kind::PowerSensor => (
            pair(
                "Transforme le niveau du signal RF simulé en mesure de puissance.",
                "Converts the simulated RF signal level into a power measurement.",
            ),
            "P_sensor[dBm] = P_RF",
            pair(
                "Générateur RF → Power Sensor → Power Meter.",
                "RF generator → Power Sensor → Power Meter.",
            ),
            pair(
                "Ni calibration du capteur ni réponse fréquentielle physique.",
                "No sensor calibration or physical frequency response.",
            ),
        ),
        Kind::PowerMeter => (
            pair(
                "Lit la puissance produite par un capteur.",
                "Reads power produced by a sensor.",
            ),
            "P_meter[dBm] = P_sensor",
            pair(
                "Connecter POWER du capteur à POWER du mesureur.",
                "Connect sensor POWER to meter POWER.",
            ),
            pair(
                "Aucune acquisition indépendante ; il faut un capteur en amont.",
                "No independent acquisition; an upstream sensor is required.",
            ),
        ),
        Kind::Python => (
            pair(
                "Transforme une trace avec un script Python et délègue les commandes SCPI au moteur Rust.",
                "Transforms a trace with Python and delegates SCPI commands to Rust.",
            ),
            "output = {'frequency_hz': [...], 'amplitude_dbm': [...], 'simulated': bool}",
            pair(
                "output = dict(trace)\noutput['amplitude_dbm'] = [x + 0.5 for x in trace['amplitude_dbm']]",
                "output = dict(trace)\noutput['amplitude_dbm'] = [x + 0.5 for x in trace['amplitude_dbm']]",
            ),
            pair(
                "Python 3.10+ ; supervision 5 s. Pas à pas au niveau bloc, pas de debugger Python intégré.",
                "Python 3.10+; 5 s supervision. Block-level stepping, no embedded Python debugger.",
            ),
        ),
        Kind::Peak => (
            pair(
                "Recherche le point de puissance maximal d'une trace.",
                "Finds the highest-power point in a trace.",
            ),
            "P_peak = max(trace.amplitude_dbm)",
            pair(
                "Analyseur TRACE → Détection de pic → Contrôle de limites.",
                "Analyzer TRACE → Peak detector → Limit check.",
            ),
            pair(
                "Pic discret ; pas d'interpolation ni d'incertitude.",
                "Discrete peak; no interpolation or uncertainty.",
            ),
        ),
        Kind::Limit => (
            pair(
                "Évalue un intervalle de puissance et produit PASS ou FAIL.",
                "Evaluates a power interval and produces PASS or FAIL.",
            ),
            "PASS ⇔ lower_dBm ≤ P ≤ upper_dBm",
            pair(
                "Pic −13 dBm, limites [−15, −11] dBm : PASS.",
                "Peak −13 dBm, limits [−15, −11] dBm: PASS.",
            ),
            pair(
                "Limites inclusives en dBm ; aucune règle multivariée ou budget d'incertitude.",
                "Inclusive dBm limits; no multivariate rule or uncertainty budget.",
            ),
        ),
    };
    Doc {
        purpose: purpose.into(),
        formula,
        example,
        limits,
    }
}
fn dsp_document(op: rf_core::dsp::Op) -> Doc {
    use rf_core::dsp::Op::*;
    let (purpose, formula, limits) = match op {
        IqSource => (
            "Porteuse complexe continue, amplitude RMS et cadence explicites.",
            "x[n] = A exp(j 2π f n/Fs)",
            "Fréquence sous Nyquist ; données simulées en V.",
        ),
        BitSource => (
            "Bits déterministes avec seed, renouvelés à chaque acquisition.",
            "SplitMix64(seed + sequence) & 1",
            "Source simulée ; nombre de bits compatible avec le code/modem requis.",
        ),
        IoSource | IoSink => (
            "Acquisition ou sortie via HAL. Choisir backend, endpoint et timeout.",
            "I/Q + Fs + fc + unité + TimeTag",
            "RAW natif indexé ; VISA cf32 LE ; SDK externes requis pour SDR/audio/ZMQ/HDF5/Parquet. Sorties physiques FS <= 1.",
        ),
        Fir => (
            "Filtre complexe à réponse impulsionnelle finie, état conservé entre trames contiguës.",
            "y[n] = Σ taps[k] x[n-k]",
            "Taps fournis par l'utilisateur ; reset en cas de gap ou changement de paramètres.",
        ),
        Iir => (
            "Filtre rationnel B/A en forme directe, avec historique entre trames.",
            "a0 y[n] = Σ b[k] x[n-k] - Σ a[k] y[n-k]",
            "La stabilité dépend des coefficients ; résultats non finis refusés.",
        ),
        Decimate | Interpolate => (
            "Changement entier de cadence avec filtre FIR configuré.",
            "decimation : Fs/L ; interpolation : Fs*L avec insertion de zéros",
            "Concevoir les taps anti-alias/anti-images pour le facteur demandé. Aucun filtre optimal implicite.",
        ),
        Fft => (
            "FFT radix 2 centrée, niveaux par bin référencés à 1 V ou 1 FS.",
            "20 log10(|FFT(x w)/N|)",
            "dBV/dBFS, jamais dBm sans impédance/calibration ; moyenne des segments disponibles.",
        ),
        Window => (
            "Fenêtrage Hann, Hamming, Blackman ou Rectangular.",
            "y[n] = w[n] x[n]",
            "Gain et ENBW varient selon la fenêtre ; PSD normalise son énergie.",
        ),
        Agc => (
            "Contrôle de gain vers amplitude RMS cible.",
            "P = (1-b)P + b|x|² ; gain = A/sqrt(P)",
            "Gain borné ; démarrage transitoire, sans attaque/relâchement séparés.",
        ),
        Pll | Costas => (
            "Boucle de phase du second ordre : porteuse PLL ou Costas BPSK/QPSK.",
            "ω += b² e ; phase += ω + 2b e",
            "Costas : ordre 2/4. Ambiguïtés de phase et capture non garanties pour toute modulation.",
        ),
        SymbolTiming => (
            "Récupération Gardner avec interpolation linéaire et cadence symbole déclarée.",
            "e = Re((early-late) conj(mid))",
            "Prototype par trame, sps >= 2 ; continuité du timing fractionnaire intertrame à développer.",
        ),
        FrameSync => (
            "Recherche exacte du préambule, correction d'inversion et extraction d'un paquet.",
            "preamble match → packet_bits bits",
            "Ni CRC ni synchronisation glissante de préambule entre deux trames.",
        ),
        DigitalMod | DigitalDemod => (
            "ASK/PSK/QAM Gray, FSK par corrélation de tons, OFDM avec CP N/8.",
            "LLR max-log = (d1²-d0²)/variance ; positif=bit 0",
            "Pulses rectangulaires, synchronisation connue. OFDM : tous les bins, sans pilotes/égaliseur. FSK fournit des décisions dures avec LLR de confiance fixe.",
        ),
        AnalogMod | AnalogDemod => (
            "AM, FM ou PM sur composante I réelle.",
            "AM=1+A m ; PM=exp(j A m) ; FM phase+=2π Δf m/Fs",
            "AM sans surmodulation ; FM sans saut de phase >= π ; pas de stéréo FM.",
        ),
        ConvEncode | ConvDecode => (
            "Convolutionnel K=3 (7,5), rate 1/2 ; Viterbi souple terminé à zéro.",
            "2 bits de terminaison ; treillis 4 états",
            "Profil de recherche, sans puncturing ni équivalence implicite à K=7.",
        ),
        RsEncode | RsDecode => (
            "RS systématique GF(256), polynôme 0x11d, racines α^0...α^(p-1).",
            "RS(255,255-p) ; t=floor(p/2)",
            "Octets MSB-first, blocs complets. Pas d'erasures ; au-delà de t, une erreur peut rester indétectée.",
        ),
        LdpcEncode | LdpcDecode => (
            "LDPC systématique de référence (128,64), décodage min-sum normalisé.",
            "H=[A|I] ; A[r]={r,r+7,r+23} mod 64",
            "Profil propre au projet, pas DVB/5G. Non-convergence explicitement signalée.",
        ),
        TurboEncode | TurboDecode => (
            "Deux RSC à 4 états, rate 1/3 ; décodage max-log BCJR itératif.",
            "LLR extrinsèques ; interleaver par inversion d'ordre",
            "Prototype non terminé, interleaver de recherche ; aucun profil 3GPP, limite 8192 bits.",
        ),
        Power => (
            "Puissance issue de l'amplitude RMS complexe calibrée et de l'impédance.",
            "P=mean(|x|²)/R ; dBm=10 log10(P/1mW)",
            "V requis ; FS est refusé. Convention RMS complexe explicite, aucune conversion RF peak/RMS implicite.",
        ),
        Snr | Evm => (
            "Erreur par rapport à une référence I/Q alignée.",
            "SNR=10 log10(Σ|ref|²/Σ|x-ref|²) ; EVM=100 sqrt(error/signal)",
            "Même Fs/fc/unité/indices/horloge/epoch. Aucune correction implicite de gain, phase ou retard.",
        ),
        Thd => (
            "THD par projection cohérente de la fondamentale et des harmoniques 2 à 10.",
            "THD%=100 sqrt(Σ|harm|²/|fund|²)",
            "Acquisition cohérente requise ; harmoniques au-delà de Nyquist omises, pas THD+N.",
        ),
        Ber | Per => (
            "Taux d'erreur de bits ou de paquets par comparaison à une référence.",
            "BER=bits erronés/N ; PER=paquets erronés/Npaquets",
            "Longueurs identiques ; PER requiert des paquets complets, sans CRC implicite.",
        ),
        PhaseNoise => (
            "Déroulement de phase, retrait de pente porteuse et PSD des fluctuations.",
            "L(f) ≈ Sφ,one-sided(f)/2 = Sφ,two-sided(f)",
            "Approximation petites phases, offsets positifs en dBc/Hz ; pas de qualification métrologique ni cross-correlation.",
        ),
        Vswr => (
            "VSWR maximal sur une acquisition de réflexion VNA.",
            "VSWR=(1+|Γ|)/(1-|Γ|)",
            "S11 ou S22 uniquement, |Γ| < 1. Le buffer du VNA connecté est utilisé.",
        ),
        Psd | Spectrogram => (
            "Welch 50 % d'overlap et waterfall sur les acquisitions exécutées.",
            "PSD=mean(|FFT(xw)|²)/(Fs Σw²)",
            "Spectre complexe deux côtés en V²/Hz ou FS²/Hz. Historique UI limité à 64 lignes.",
        ),
        SpectralPeak => (
            "Fréquence du bin spectral de niveau maximal.",
            "f[argmax(levels)]",
            "Pic discret unique, sans interpolation ni liste de pics.",
        ),
        DcOffset => (
            "Suppression de la moyenne complexe de la trame.",
            "y=x-mean(x)",
            "Retire aussi un signal utile situé exactement à DC.",
        ),
        IqBalance => (
            "Correction affine du DC et du déséquilibre gain/phase I/Q.",
            "I=Iraw-DCI ; Q=((Qraw-DCQ)/gain - I sinθ)/cosθ",
            "Paramètres fournis/calibrés ; aucun estimateur aveugle implicite.",
        ),
        Cable => (
            "Correction du gain et de la phase porteuse d'un câble.",
            "y=x 10^(loss/20) exp(j2π fc delay)",
            "Approximation bande étroite ; pas de filtre de retard fractionnaire de l'enveloppe.",
        ),
        CalibrationTable => (
            "Interpolation linéaire des corrections gain/phase à la fréquence centrale.",
            "[Hz,dB,deg] ; volts_per_fs optionnel",
            "Aucune extrapolation ; volts_per_fs est un facteur RMS fourni explicitement. Pas de réponse en fréquence par bin.",
        ),
        Uncertainty => (
            "Incertitude élargie à partir de composantes standard indépendantes.",
            "U=k sqrt(Σuᵢ²)",
            "Même unité que l'entrée ; pas de covariance, degrés de liberté ou propagation automatique du graphe.",
        ),
        Convert => (
            "Conversions dimensionnelles et logarithmiques avec impédance déclarée.",
            "dBm↔W↔Vrms↔Arms ; ratio↔dB ; %↔ratio",
            "Conversions incompatibles refusées. dB s'applique à un rapport de puissances ; FS exige une calibration.",
        ),
        Channel => (
            "Canal simulé avec AWGN, block fading Rayleigh, FIR multipath, Doppler et compression.",
            "y=fading*(h*x)/(1+α|h*x|²) exp(j2π Δf t)+AWGN",
            "Bruit défini par la puissance d'entrée ; fading constant par trame, sans modèle normalisé 3GPP.",
        ),
    };
    Doc {
        purpose: purpose.into(),
        formula,
        example: "RF / DSP → Charger QAM16 / AWGN / Viterbi → Exécuter. Ajouter ce bloc depuis la palette et configurer ses paramètres dans l'inspecteur.",
        limits,
    }
}
pub fn block(ui: &mut egui::Ui, kind: Kind) {
    let d = document(kind);
    ui.heading(t(kind.label()));
    ui.label(d.purpose);
    ui.separator();
    caption(ui, "Ports de données");
    for (direction, ports) in [("IN", kind.inputs()), ("OUT", kind.outputs())] {
        for p in ports {
            ui.label(format!(
                "{direction} {} · {}{}",
                p.name,
                p.port.label(),
                if p.required {
                    pair(" · requis", " · required")
                } else {
                    ""
                }
            ));
        }
    }
    ui.separator();
    caption(ui, "Formule");
    ui.label(RichText::new(d.formula).monospace());
    caption(ui, "Exemple");
    ui.label(d.example);
    caption(ui, "Limites du modèle");
    ui.label(d.limits);
}
pub fn global(ui: &mut egui::Ui) {
    let sections = [
        (
            pair("Construire un banc", "Build a bench"),
            pair(
                "Rechercher un bloc dans la palette, cliquer pour l'ajouter. La recherche accepte des lettres espacées et des alias ADC, DAC, IQ. Clic droit dans la palette pour ajouter un favori. Les huit derniers types utilisés apparaissent dans Récents.",
                "Search the palette, click to add a block. Search supports subsequences and ADC, DAC, IQ aliases. Right-click a palette item to favorite it. The last eight types appear in Recent.",
            ),
        ),
        (
            pair("Éditer et câbler", "Edit and wire"),
            pair(
                "V : sélection. Shift+clic ajoute ou retire un bloc ; glisser le fond dessine un rectangle de sélection. Ctrl+A sélectionne tout, Ctrl+C copie, Ctrl+V colle avec les connexions internes. Ctrl+D duplique la sélection. L'historique de chaque espace n'a pas de plafond fixe et dure jusqu'à sa fermeture. W : curseur croix, cliquer la source puis une broche cible compatible avec aimantation légère ; cliquer le fond ajoute des coudes ; Échap annule. Les liaisons RF/DC sont distinctes des dépendances de données. Le routage intelligent cherche un parcours orthogonal autour des blocs ; un message explique les zones trop denses ou obstruées. H ou bouton central : déplacement ; molette : zoom. Édition propose alignements, distribution et organisation topologique.",
                "V selects. Shift-click adds/removes a block; drag the background to marquee-select. Ctrl+A selects all, Ctrl+C copies, Ctrl+V pastes internal connections, Ctrl+D duplicates the selection. Each workspace has history without a fixed count limit until closed. W connects an output to an input; background clicks add bends; Escape cancels. Smart routing finds orthogonal paths around blocks; dense or blocked regions produce an explicit message. H or middle mouse pans; wheel zooms. Edit offers alignment, distribution and topological layout.",
            ),
        ),
        (
            pair("Clavier et annotations", "Keyboard and annotations"),
            pair(
                "Tab et Shift+Tab parcourent les blocs. Flèches gauche/droite choisissent un port nommé ; Entrée sélectionne la sortie ou connecte l'entrée. Ctrl+K configure plusieurs raccourcis par action. Les raccourcis restent suspendus pendant la saisie. Édition → Annotation crée une note déplaçable, éditable dans l'inspecteur. Les commentaires des blocs sont sauvegardés avec le graphe. F1 ouvre cette aide, F2 l'aide du bloc sélectionné.",
                "Tab and Shift+Tab cycle blocks. Left/right arrows choose a named port; Enter starts an output wire or connects an input. Ctrl+K configures multiple shortcuts per action. Shortcuts are suspended while typing. Edit → Annotation creates a movable note editable in the inspector. Block comments are saved with the graph. F1 opens this guide, F2 opens help for the selected block.",
            ),
        ),
        (
            pair("Organiser les vues", "Organize views"),
            pair(
                "Chaque vue d'analyse et le graphe disposent du menu Disposition : principale, à droite, en bas, fenêtre flottante ou masquée. Les panneaux droits/bas et les fenêtres se redimensionnent. Espaces de travail permet de nommer et sauvegarder ces layouts, et de créer jusqu'à huit bancs indépendants. Enregistrer Studio conserve projets, dispositions, zoom et préférences. Les buffers et historiques de session sont indépendants mais ne sont pas enregistrés sur disque. Le changement de banc est désactivé pendant une exécution.",
                "Graph and analysis panes have a Layout menu: main, right, bottom, floating or hidden. Dock regions and windows resize. Workspaces saves named layouts and holds up to eight independent benches. Save Studio preserves projects, layouts, zoom and preferences. Session buffers and histories are independent but are not saved to disk. Workspace switching is disabled while running.",
            ),
        ),
        (
            pair("Mesures RF", "RF displays"),
            pair(
                "Waterfall accumule jusqu'à 128 acquisitions compatibles, sans inventer de trames. La constellation utilise deux buffers I/Q de mêmes unités et cadence ; tous les échantillons dessinent la trajectoire, le sous-échantillonnage utilise l'offset et le nombre d'échantillons/symbole choisis. Aucun EVM ou synchroniseur n'est calculé. Eye replie le buffer sur deux intervalles unitaires, selon une cadence symbole fournie par l'utilisateur, sans récupération d'horloge. Chronogramme montre le seuillage 0/1 d'un buffer, pas un protocole logique décodé. Smith accepte S11/S22 et une impédance Z₀ explicite ; aucune calibration métrologique n'est revendiquée.",
                "Waterfall accumulates up to 128 compatible acquisitions without inventing frames. Constellation uses two I/Q buffers with matching units and rate; all samples show the trajectory, symbol decimation uses the chosen offset and samples/symbol. No EVM or synchronization is computed. Eye folds a buffer across two unit intervals at a user-specified symbol rate without clock recovery. Timing shows a thresholded 0/1 buffer, not a decoded logic protocol. Smith accepts S11/S22 with explicit reference impedance Z₀; no metrological calibration is claimed.",
            ),
        ),
        (
            pair("Déboguer le flux", "Debug the dataflow"),
            pair(
                "Cocher Breakpoint ou Sonde dans l'inspecteur, ou utiliser le clic droit du bloc. Démarrer le debug fige un instantané du banc et s'arrête avant le premier bloc. Pas à pas exécute un bloc, Continuer avance jusqu'au prochain breakpoint. Les sondes montrent les sorties et la taille originale ; la prévisualisation des tableaux est limitée à 4096 échantillons et 512 ports, avec une indication explicite. Arrêter annule même pendant une pause. Ce mode est réservé à la simulation ; il ne débogue ni l'intérieur d'un pilote ni les lignes Python.",
                "Enable Breakpoint or Probe in the inspector or block context menu. Start debugging freezes a bench snapshot and pauses before the first block. Step executes one block; Continue advances to the next breakpoint. Probes show outputs and original sizes; array previews are limited to 4096 samples and 512 ports with explicit labels. Stop cancels even while paused. This mode is simulation-only; it does not debug driver internals or Python lines.",
            ),
        ),
        (
            pair("Affichage et accessibilité", "Appearance and accessibility"),
            pair(
                "Dans Espaces de travail : thème clair/sombre, contraste renforcé, texte 85–150 %, français ou anglais. Les ports portent leurs noms et unités, les états PASS/FAIL restent textuels et les commandes ont des tooltips. AccessKit est activé pour les widgets egui ; les dessins du graphe ont une navigation clavier et un inspecteur textuel. Une certification lecteur d'écran complète et le tactile mobile restent à valider.",
                "Workspaces provides light/dark themes, high contrast, 85–150% text size, French or English. Ports carry names and units, PASS/FAIL remains textual, and commands have tooltips. AccessKit is enabled for egui widgets; graph drawings have keyboard navigation and a text inspector. Complete screen-reader certification and mobile touch support remain unvalidated.",
            ),
        ),
    ];
    for (title, text) in sections {
        ui.heading(title);
        ui.label(text);
        ui.add_space(14.);
    }
    ui.hyperlink_to(
        pair("Guide du projet", "Project guide"),
        "https://github.com/Citroz31/rf-workbench",
    );
    ui.hyperlink_to(
        pair(
            "Smith et impédance — Keysight",
            "Smith and impedance — Keysight",
        ),
        "https://helpfiles.keysight.com/csg/pxivna/Tutorials/Comp_Imped.htm",
    );
    ui.hyperlink_to(
        pair(
            "Constellation I/Q — MathWorks",
            "I/Q constellation — MathWorks",
        ),
        "https://www.mathworks.com/help/comm/ref/constellationdiagram.html",
    );
}
