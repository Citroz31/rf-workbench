# Validation 0.3.0

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
