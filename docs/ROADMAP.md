# Étapes suivantes

| Étape | Livrable vérifiable |
| --- | --- |
| 0 — Prototype Windows, livré | Éditeur de blocs, moteur DAG, simulation RF, sessions SCPI TCP, adaptateur VISA, scripts Python, tests intégrés, export |
| 0.2 — Bibliothèque et câblage, livré | 18 blocs vectoriels, ports multiples typés, parcours personnalisés, raccourcis configurables, catalogue DUT, démonstrations PNA/I/Q/thermique et vues avec unités |
| 1 — Validation sur instruments | Identification d'un générateur et d'un analyseur réels, profils SCPI par modèle, lecture ASCII/binaire avec captures de référence, arrêt RF confirmé |
| 2 — Métrologie RF | Acquisition VNA, S-paramètres complexes, import/export Touchstone, Smith chart, plans de calibration, incertitudes et limites avec unités explicites |
| 3 — Éditeur avancé | Sous-bancs, groupes, boucles contrôlées, alignement, sélection multiple, routage automatique évitant les obstacles, débogage pas-à-pas, gestion de plusieurs traces |
| 4 — Production desktop | Tests graphiques automatisés, profils de 100/1000 blocs, DPI multiples, accessibilité, installateurs signés, sauvegarde automatique et récupération |
| 5 — Linux/macOS | Validation utilisateur du rendu et des pilotes, packages et différences de runtime VISA ; compilation et tests déjà couverts par la CI |
| 6 — Agent de banc distant | API de sessions distante authentifiée, commande/capture indépendante du client, contrôle exclusif de l'instrument et arrêt vérifiable |
| 7 — Android/iOS | Hôtes mobiles, ergonomie tactile, permissions, client de l'agent distant, tests sur appareils et distribution |

Pour le prochain incrément, relever les modèles exacts des instruments, leurs interfaces (LAN SOCKET, VXI-11, HiSLIP, USB, GPIB), le runtime VISA déjà installé et un scénario de mesure prioritaire. Ces informations permettront de valider un driver réel sans supposer une compatibilité SCPI universelle.
