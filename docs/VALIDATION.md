# Validation de la première livraison

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
