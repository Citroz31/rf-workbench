# Architecture

Objectif : assembler, exécuter et analyser un banc RF dans une interface manipulable pendant les acquisitions. Le cas de référence est une source à 2,45 GHz, un DUT, une acquisition de spectre, une compensation Python, une extraction de pic et un contrôle de limites.

```mermaid
flowchart LR
  UI[Interface native Rust / egui] -->|commandes| W[Worker de banc]
  W --> E[Moteur de graphe typé]
  E --> S[Sessions Rust]
  S --> SIM[Simulateur RF]
  S --> TCP[SCPI TCP]
  S --> VISA[Runtime VISA constructeur]
  E <-->|JSON lines supervisé| PY[Processus Python]
  PY -->|query / write via Rust| S
  W -->|dernière acquisition complète| UI
```

## Dépendances des composants

| Crate | Responsabilité | Dépend de |
| --- | --- | --- |
| `rf-core` | Unités, trace, graphe DAG, ports typés, format de projet, résultats de test | Serde / thiserror |
| `rf-instruments` | ResourceManager, sessions, SCPI TCP, simulateur, VISA dynamique, blocs IEEE 488.2 | rf-core / libloading |
| `rf-runtime` | Worker, exécution, annulation, contrôle des sorties, supervision Python | rf-core / rf-instruments |
| `rf-workbench` | Schéma, navigation, inspecteur, historique, courbes, éditeur, fichiers | Tous les composants précédents / eframe |

## Contrat d'exécution

Chaque bloc dispose au plus d'une entrée et d'une sortie, avec branchement de sortie vers plusieurs consommateurs possible. Les types sont `Signal` (description du signal RF), `Trace` et `Scalar` (dBm). Les connexions déterminent l'ordre topologique ; l'ordre d'affichage et les identifiants n'imposent pas l'exécution. Les cycles, entrées multiples, références absentes et ports incompatibles sont rejetés. Les entrées nécessaires doivent être reliées avant exécution.

Le générateur produit fréquence, niveau et provenance ; le DUT transforme le niveau théorique ; l'analyseur produit une trace simulée ou acquise selon sa ressource. Python transforme la trace et le moteur valide sa sortie. Le pic produit un scalaire ; le bloc limite produit un résultat PASS/FAIL. Le prototype affiche la dernière trace produite par l'ordre d'exécution. La gestion simultanée de plusieurs traces étiquetées reste à implémenter.

Un démarrage prend une copie du graphe. Le worker possède les sessions : aucun accès instrument ni processus Python ne se déroule dans la boucle graphique. Un seul travail de banc est accepté à la fois. Les commandes et événements sont bornés ; les traces passent par un seul emplacement remplaçable. La consommation mémoire des acquisitions ne croît pas lorsque le rendu ralentit. L'arrêt utilise un drapeau atomique ; les transferts bloquants sont limités par le transport, pas interrompus arbitrairement au milieu d'une commande.

La boucle graphique dessine les blocs visibles et limite le nombre de points de courbe à un budget proportionnel aux pixels. La décimation conserve les extrema, afin de garder les pics étroits. Le rendu utilise la synchronisation verticale et se réveille lors des événements ; en cours d'acquisition, un rafraîchissement est demandé toutes les 16 ms. Ce sont des choix de conception, pas une garantie temps réel ni une preuve de 60 images/s pour des graphes arbitraires. Le mutex de résultat ne protège que le déplacement d'une trace complète.

## Contrat Python

Le worker lance l'interpréteur choisi avec un runner embarqué dans le binaire. Le script et la trace initiale sont envoyés sur stdin en JSON. La sortie stdout est réservée aux messages `query`, `write`, `result`, `error`. Rust traite les demandes d'instruments et répond sur stdin. Le script est limité à 100 opérations de passerelle ; les messages sont bornés à 4 Mo. Le processus est terminé et récolté sur les chemins de succès, erreur, arrêt et délai. Les processus descendants éventuellement lancés par le script ne sont pas gérés : ce n'est pas un bac à sable.

Python ne peut pas changer l'étiquette de provenance d'une trace simulée en donnée matérielle dans le chemin normal de résultat. La validation porte sur les données, pas sur l'exactitude du calcul utilisateur. L'environnement numérique appartient au métrologue et peut inclure NumPy, SciPy ou des librairies métier.

## Portabilité

Les crates de données et de sessions évitent l'API Windows dans leurs interfaces publiques. La façade VISA contient l'FFI, avec types fixes et durée de vie de la bibliothèque liée à celle des handles. Le masquage de fenêtre du processus Python est conditionné à Windows. L'application native egui est prévue pour Windows/Linux/macOS ; les variantes d'empaquetage et les dépendances système sont différentes.

Pour Android/iOS, partager d'abord `rf-core` et les composants de rendu, puis créer un hôte mobile pour le cycle de vie, les permissions et l'accès aux fichiers. Le pilotage devrait initialement passer par un agent de banc desktop distant, plutôt que de supposer la présence d'un runtime VISA ou de Python sur le téléphone. Le protocole distant, l'authentification et le transport chiffré restent à concevoir : aucun serveur réseau n'est exposé par cette version.

## Limites du prototype

Le moteur est séquentiel ; il n'inclut pas de boucles de programme, sous-schémas, branchements conditionnels ou scheduler temps réel. Les sessions VISA sont implementées mais non éprouvées sur le runtime constructeur de cette machine. Les drivers SCPI restent génériques. Le système ne fournit pas de budget d'incertitude, certificat de calibration, traçabilité ISO/IEC 17025 ni validation de conformité RF. L'historique contient le graphe et ses paramètres ; les résultats restent en mémoire et peuvent être exportés en CSV.
