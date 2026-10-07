# Validation 0.7 — projets, PNA et fixtures

Contrôles locaux Windows x64, Rust 1.90, 7 octobre 2026.

- Formatage et Clippy strict workspace/all-targets réussis ; **130 tests Rust** et **5 intégrations Python** réussis.
- Reset du worker : une sortie DC simulée activée est coupée, les sessions sont fermées, la séquence repart à zéro ; projets historiques et nouveaux champs relus.
- Flow : bandes incompatibles, niveaux d'entrée/sortie déclarés et puissance de sweep contrôlés. OP1dB produit un avertissement de compression, pas une limite de dommage inventée.
- PNA : matrice S d'un seul balayage HOLD, refus avant émission RF si une trace manque, niveaux interrogés avant écriture, axe de puissance signé et unités séparées de l'axe Hz.
- Touchstone RI/MA/DB, reverse, cascades/deembedding complexes, grilles singulières/non compatibles et refus d'un 2xThru 36–38 GHz seuls vérifiés.
- Extraction native comparée aux fixtures complexes de **scikit-rf 1.8.0 / IEEEP370_SE_NZC_2xThru**, transitions 53/47 Ω, plan de coupe forcé 50 Ω ; erreur complexe maximale sous **2e-5**. L'oracle est un outil de développement ; aucun Python n'est nécessaire dans l'application.
- Sweep puissance simulé : perte de gain 1 dB à Pin = OP1dB − gain + 1 dB, sans présenter ce modèle comme une courbe constructeur.
- Six captures natives 1600 × 1000 inspectées : accueil, projet, PNA, résultats, fixtures, flow. Le guide de 32 chapitres est rendu et contrôlé sur toutes ses pages.

Ces tests ne qualifient pas un N5245B physique, son firmware, ses licences ni ses calibrations. Le solveur RF n'est pas un simulateur électromagnétique, et les limites inconnues restent signalées. L'extraction NZC suppose des demi-fixtures de longueurs électriques égales ; elle n'est pas une certification IEEE 370, un ZC complet ou l'AFR propriétaire Keysight. Les applications NF/IMD/GCA utilisent les canaux préparés sur l'instrument.

La livraison Windows MSVC doit réussir ses tests et vérifier le ZIP et l'EXE par téléchargement anonyme avant d'être annoncée. Les historiques ci-dessous décrivent leurs versions respectives.

# Validation 0.5.0 - interface, projets et PNA

## Version 0.6 - Windows local, 7 octobre 2026

- `cargo fmt` et `cargo clippy --workspace --all-targets -- -D warnings` réussis.
- **119 tests Rust** réussis, puis **5 tests d’intégration Python** réellement exécutés (processus, SCPI, erreurs, timeout et Stop). Les suites sans tests ne sont pas comptées.
- 20 autotests du binaire release réussis avec PATH vide et Python indisponible, depuis `C:/Windows/System32`, données dans un dossier utilisateur.
- Deux fichiers `.rfbench` effectivement chargés et simulés : MAAL gain 26 dB, CGY gain 5,8 dB, S21 et 401 points, provenance simulée.
- Test de souris egui : clic source, prévisualisation près du pin, absence de création au survol, clic cible, undo/redo, persistance W ; refus de pin occupé, incompatible ou éloigné, marge conservée avec le zoom.
- Tests des boucles physiques indépendantes du DAG, réduction de ports, sérialisation historique et copier/coller ; simulation RX/TX, 31,5 dB d’atténuation et 354,375° de phase.
- Tests SCPI PNA : ports réels insuffisants, trace S43 sur 4 ports, identité USB VNA ; tests DC : identité erronée, couplage, consignes divergentes, OFF avant réglage, ON explicite, annulation et Stop. Le simulateur E3631A conserve un interrupteur global.
- Cinq captures natives 1600 × 1000 inspectées : schéma, fenêtre PNA, DUT CGY, DC et catalogue. Aucun benchmark pointeur-écran ou essai sur écran tactile/DPI multiple n’est revendiqué.

Aucun instrument physique n’a été connecté. Les réponses contrôlées vérifient le protocole, pas le comportement de chaque firmware ni une qualification métrologique. Les modèles DUT n’incluent ni compression, ni bruit NF généré, ni courbes constructeur mesurées. Les câbles DC ne séquencent pas la polarisation.

La publication Windows MSVC et les téléchargements publics sont vérifiés par le workflow `release.yml` pour le tag livré. Le succès local GNU n’est pas présenté comme une validation Linux/macOS/mobile.

Contrôles locaux sur Windows x64, Rust 1.90.0, Python 3.12.14, le 7 octobre 2026.

| Vérification | Résultat |
| --- | --- |
| Tests Rust workspace | 102 réussis, dont 7 tests PNA et les tests de nouveaux projets, gain PA, adresse unique et resize graphique |
| Intégration Rust/Python | 5 réussis avec un vrai processus Python, y compris annulation et timeout |
| Formatage / Clippy all-targets `-D warnings` | Réussis |
| Compilation Windows release | Réussie |
| Autotests disponibles dans l'application | 20 réussis |
| Rendu natif 1600 × 1000 | Accueil, fenêtre PNA (Connexion/Fonctions), navigateur de projets et banc PA inspectés |
| Guide utilisateur | 24 pages A4, rendues et inspectées ; texte, pagination et bornes des mots vérifiés sur toutes les pages |
| Archive Windows / sources | CRC, présence du PDF/exemples et correspondance du binaire/commit vérifiées lors de l'empaquetage |

Les tests PNA vérifient SDATA REAL32/64, axe REAL64, channel/nom de mesure, absence de trigger par défaut, identité attendue, limites de sweep, classes autorisées, FDATA/GCA avec relecture, payloads invalides et restauration après erreur (y compris erreur simultanée de restauration). Un serveur SCPI TCP local transmet les blocs IEEE 488.2 par fragments de trois octets. Le test egui de redimensionnement injecte un vrai déplacement de pointeur dans le coin du graphique et constate sa nouvelle hauteur.

Le projet PA vérifie un gain petit signal de 20 dB ± 0,15 dB sur 36–38 GHz ; il ne simule ni saturation ni P1dB. La courbe de compression du guide et son CSV sont explicitement synthétiques. Le guide suppose OP1dB = 34 dBm en sortie, donc Pin1dB = 15 dBm pour un gain comprimé de 19 dB.

Aucun runtime VISA ni instrument du laboratoire n'a été utilisé ici. GPIB/USB/LAN physiques, débits binaires, firmware/licences, calibrations et arrêt RF nécessitent une validation matérielle. La détection des classes et la lecture FDATA ne constituent pas une implémentation complète des applications GCA/NF/spectre/IMD. Les tests de pointeur et captures ne prouvent pas une latence garantie, tous les DPI ni une ergonomie tactile. Linux/macOS sont testés par la matrice [GitHub Actions](https://github.com/Citroz31/rf-workbench/actions) après publication ; iOS/Android ne sont pas empaquetés.

Les contrôles Python 0.4 restent applicables : le protocole Python n'a pas changé dans cette version. Les résultats CI du commit de livraison se consultent dans GitHub Actions ; leur simple configuration ne prouve pas leur réussite.

# Validation 0.4.0 — RF/DSP et HAL

Vérifications locales sur Windows x64, Rust 1.90.0 et Python 3.12.14, le 7 octobre 2026. L'exécutable Windows est construit à partir de ces sources.

| Vérification | Résultat |
| --- | --- |
| Tests Rust workspace | 90 réussis : 18 tests DSP numériques, 8 HAL, 4 graphes DSP, plus 60 tests existants |
| Intégration Rust/Python | 5 réussis avec un vrai interpréteur : commandes, compensation, erreurs, annulation et timeout |
| Tests Python de la passerelle | 9 réussis ; couverture lignes/branches 100 % de rfworkbench/runner.py |
| Adaptateurs Python optionnels localement | 1 test réussi, 5 ignorés faute de h5py/pyarrow/pyzmq ; la CI installe ces dépendances et exécute les 6 tests |
| Formatage / Clippy tous les targets, warnings refusés | Réussis |
| Ruff (y compris adapters), ty et basedpyright (passerelle typée) | Réussis ; scripts SDK dynamiques testés séparément |
| Release Windows GNU | Réussie |
| Autotests de l'application | 20 réussis |
| Rendu natif | Vue I/Q, constellation, spectre/waterfall avec 8 acquisitions et schéma des 11 blocs inspectés à 1600 × 1000 |
| Banc QAM16 AWGN à 30 dB | Premier cycle : BER 0, EVM 3,1792 %, SNR 29,9536 dB ; provenance simulée |
| Benchmark de 1000 cycles DSP | 492,68 ms au total ; dernière exécution 0,4374 ms |

Le benchmark mesure onze blocs simulés, 1024 bits, convolutionnel K=3, QAM16/AWGN/Viterbi, PSD FFT256 et quatre mesures. Il exclut rendu, I/O physiques et lancement de SDK. Il ne mesure pas la latence pointeur→écran et ne garantit pas une cadence temps réel.

Les tests numériques vérifient FFT/inverse, bin connu, intégrale de PSD, continuité FIR entre trames, cadence de décimation, modems ASK/FSK/PSK/QAM/OFDM, voisinage Gray, AM/FM/PM, THD cohérente, phase noise/axes, frame sync, correction Viterbi, huit erreurs RS, correction souple LDPC/Turbo, AWGN/SNR/EVM, unités/SI, calibration I/Q et conversion explicite FS→V. Ils refusent références nulles, tailles excessives et timestamps débordants. Ils ne constituent pas une qualification métrologique ni une validation statistique complète des modems/codes.

Les tests HAL utilisent des fichiers et sockets locaux réels : index/provenance RAW et fichier tronqué, trames TCP fragmentées/écho, timeout total face à une en-tête envoyée lentement, UDP/métadonnées, file SPSC avec 100000 transferts, overflows/discontinuités du pump et acquisition VNA typée sur serveur SCPI de test. Les graphes vérifient la référence BER/SNR, les 45 types sérialisés, le mode matériel et le VSWR connecté à son propre VNA en présence de plusieurs acquisitions.

La découverte VISA est compilée, mais aucun runtime VISA n'est installé sur cet environnement et aucun instrument physique n'a été contacté. GPIB/USB, SDK SDR, audio, horloges externes, triggers, PTP/GPSDO/MIMO et budgets d'incertitude restent à valider/compléter sur matériel. La [matrice 0.4](V0.4.md) distingue implémentations natives, adaptateurs optionnels, profils de recherche et points d'extension.

La CI 0.3 a réussi sur Windows, Linux, macOS et Python. Les six tests HDF5/Parquet/ZMQ de la première publication 0.4 ont réussi en CI, ainsi que macOS et Python. La CI du commit final est consultable dans [GitHub Actions](https://github.com/Citroz31/rf-workbench/actions). La configuration de la CI seule ne prouve pas sa réussite.

# Historique : validation 0.3.0

Vérifications locales sur Windows x64, Rust 1.90.0 et Python 3.12.14, le 7 octobre 2026. Le binaire fourni est compilé en release depuis les sources 0.3.

| Vérification | Résultat |
| --- | --- |
| Tests Rust du workspace | 60 réussis : graphe/annotations, transports, modèles RF, debug, éditeur, analyses et persistance |
| Tests Rust/Python avec un vrai processus Python | 5 réussis, lancés explicitement après la suite Rust |
| Tests Python | 9 réussis ; couverture lignes et branches de la passerelle à 100 % |
| Formatage Rust et Clippy tous les targets avec `-D warnings` | Réussis |
| Ruff, format Python, ty et basedpyright | Réussis ; zéro erreur/avertissement |
| Compilation Windows release | Réussie |
| `--self-test` | 14 contrôles intégrés réussis |
| `--python-smoke` | IPC, SCPI et compensation : PASS |
| Rendu natif 1600 × 1000 | Graphe/spectre/debug, galerie, constellation, Smith, waterfall, œil, chronogramme, aide et thème clair anglais inspectés |

La suite de 74 tests (60 Rust + 5 Rust/Python + 9 Python) vérifie notamment : pause avant le premier bloc, pas unique, arrêt sur breakpoint, poursuite jusqu'à la fin, annulation en pause et refus de toute ressource physique en debug ; limites/provenance des buffers ; réflexion et impédance complexes, refus de S21, compatibilité I/Q, repliement temporel, changement de grille et borne de la waterfall ; restauration de la persistance et defaults des projets 0.2.

Les tests egui injectent des événements réels de pointeur pour la sélection Shift+clic et le déplacement groupé, puis vérifient une seule annulation. Les événements clavier Tab/Entrée créent une connexion par ports nommés en présence d'un bouton de barre d'outils ; le focus des champs de texte suspend les raccourcis. Un test conserve et annule 150 éditions, au-delà de l'ancienne limite de 100. Le routage est vérifié contre un obstacle et dans son thread, avec conservation des parcours manuels et de l'instantané initial. Les tests ne constituent pas une campagne utilisateur de toutes les opérations graphiques.

La capture waterfall contient huit acquisitions effectivement reçues. La constellation provient des buffers AWG I/Q ; l'œil est un repliement de cette forme d'onde, sans source numérique ni synchronisation ajoutée. Le Smith utilise le S11 du modèle PNA-X. La capture debug montre la pause réelle avant AWG, et la capture de Studio montre l'inspection des buffers après une exécution. Les captures sont produites par le framebuffer natif de l'application, pas par une maquette web.

Les contrôles locaux portent sur un DPI et une résolution. Les lecteurs d'écran, périphériques tactiles, profils de 100/1000 blocs, latence pointeur→écran, instruments physiques et temps réel restent non validés. Les calculs d'analyse sont des outils de prototype, sans qualification métrologique. L'historique de session consomme de la mémoire et n'est pas persistant. Les résultats de la matrice CI du commit 0.3 sont consultables dans [GitHub Actions](https://github.com/Citroz31/rf-workbench/actions) ; les versions précédentes disposent de builds Windows/Linux/macOS.

# Historique : validation 0.2.0

Vérifications locales sur Windows x64, Rust 1.90.0 et Python 3.12.14, le 7 octobre 2026. Le paquet Windows est compilé depuis ces sources.

| Vérification | Résultat |
| --- | --- |
| Tests Rust du workspace | 40 réussis : graphe multiport, unités, modèles I/Q/PNA, catalogue DUT, câblage, historique, raccourcis et transports |
| Tests Rust/Python avec un vrai processus Python | 5 réussis : SCPI délégué, compensation, erreurs, annulation et délai |
| Tests Python | 9 réussis ; couverture lignes/branches 100 % de la passerelle |
| Formatage, Clippy tous les targets avec `-D warnings` | Réussis |
| Ruff, ty et basedpyright | Réussis |
| Compilation Windows release | Réussie |
| `--self-test` | 14 contrôles intégrés réussis |
| `--python-smoke` | Passerelle IPC, SCPI et compensation réussis |
| Rendu natif 1600 × 1000 | Schéma, galerie des 18 blocs, paramètres S, formes d'onde et préférences inspectés |

Le test de câblage injecte des événements pointeur dans les régions de clic réelles d'egui : sortie → fond du canevas pour un coude → entrée. Il vérifie la connexion, son parcours et l'historique. Les tests vérifient également les connexions incompatibles et la suspension des raccourcis dans les champs de texte. Cette vérification automatisée ne remplace pas une campagne utilisateur sur différents DPI ou périphériques tactiles.

Les nouvelles simulations conservent leurs unités et leur provenance. Les profils matériels non implémentés sont refusés avant toute commande physique. Aucun PNA, AWG, capteur ou appareil thermique réel n'a été connecté. Le catalogue DUT initial ne contient aucune caractéristique de puce ; le scraping constructeur reste à développer.

Les captures mesurent un rendu natif, sans promettre de latence pointeur→écran ou de temps réel. Les résultats CI de 0.2 sont consultables dans GitHub Actions après publication. La CI de la livraison précédente a réussi sur Windows, Linux et macOS.

# Historique : validation de la première livraison

Vérifications locales sur Windows x64, Rust 1.90.0, compilation GNU optimisée, Python 3.12.14. Livraison préparée le 7 octobre 2026 (heure Europe/Paris).

| Vérification | Résultat |
| --- | --- |
| `cargo test --workspace --locked` | 22 tests Rust réussis ; les 5 tests Python sont exclus de cette commande et lancés séparément |
| `cargo test -p rf-runtime --test python_integration --locked -- --ignored` | 5 réussis : échanges SCPI, compensation, validation, provenance, erreur, annulation et délai Python |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Réussi, aucun avertissement |
| `cargo fmt --all -- --check` | Réussi |
| `uv run pytest --cov --cov-branch` | 9 tests Python réussis ; couverture lignes et branches 100 % pour la passerelle |
| `uv run ruff check .`, `uv run ruff format --check .` | Réussis |
| `uv run ty check`, `uv run basedpyright` | Réussis ; aucune erreur ni avertissement |
| Compilation Windows release | Réussie ; exécutable autonome d'environ 8 Mo, dépendances DLL système uniquement |
| `--self-test` | 11 contrôles intégrés réussis |
| `--python-smoke --python …` | SCPI délégué à Rust et compensation Python : pic -12,5 dBm au lieu de -13 dBm |
| Capture native du rendu | Inspectée à 1600 × 1000 ; panneaux, inspector, schéma et spectre lisibles, thème sombre corrigé |

Le benchmark effectue 1 000 cycles de cinq blocs, sans le bloc Python, avec une trace de 401 points. Deux mesures locales : 41,33 ms puis 34,84 ms au total (environ 0,035–0,041 ms par cycle), pic attendu à 2,45 GHz / -13 dBm et limites PASS. Ces chiffres mesurent le moteur simulé, sans affichage, sans trafic d'instrument et sans lancement de Python. Ils ne mesurent pas la latence pointeur→écran, ne garantissent pas une cadence temps réel, et ne constituent pas une validation sur instruments physiques.

Les tests réseau utilisent des serveurs locaux : réponses texte fragmentées, suivi d'une lecture binaire par une lecture texte, délai et refus d'une session désynchronisée. Aucun instrument du laboratoire n'a été contacté. Le backend VISA a été compilé et ses statuts vérifiés ; le runtime constructeur, ses transferts et la coupure physique RF restent à valider.

La validation interactive de l'application par automatisation Windows n'a pas été terminée : l'autorisation de lancement a expiré. La capture a été produite par le rendu natif de l'application elle-même. Aucun test manuel de déplacement, de connexion ou de DPI multiple n'est déclaré réussi.

Linux/macOS disposent d'une matrice CI mais n'ont pas été exécutés localement. iOS/Android n'ont pas de paquet exécutable dans cette livraison. Les résultats GitHub Actions doivent être lus dans le dépôt après publication ; la présence de la configuration ne prouve pas leur réussite.
