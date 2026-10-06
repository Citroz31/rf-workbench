# Étapes suivantes

| Étape | Livrable vérifiable |
| --- | --- |
| 0 — Prototype Windows, livré | Éditeur de blocs, moteur DAG, simulation RF, sessions SCPI TCP, adaptateur VISA, scripts Python, tests intégrés, export |
| 1 — Validation sur instruments | Identification d'un générateur et d'un analyseur réels, profils SCPI par modèle, lecture ASCII/binaire avec captures de référence, arrêt RF confirmé |
| 2 — Métrologie RF | Acquisition VNA, S-paramètres complexes, import/export Touchstone, Smith chart, plans de calibration, incertitudes et limites avec unités explicites |
| 3 — Éditeur avancé | Sous-bancs, groupes, ports multiples, boucles contrôlées, alignement, sélection multiple, routage des câbles, débogage pas-à-pas, gestion de plusieurs traces |
| 4 — Production desktop | Tests graphiques automatisés, profils de 100/1000 blocs, DPI multiples, accessibilité, installateurs signés, sauvegarde automatique et récupération |
| 5 — Linux/macOS | Exécution des jobs CI, validation de rendu et de pilotes, packages et différences de runtime VISA |
| 6 — Agent de banc distant | API de sessions distante authentifiée, commande/capture indépendante du client, contrôle exclusif de l'instrument et arrêt vérifiable |
| 7 — Android/iOS | Hôtes mobiles, ergonomie tactile, permissions, client de l'agent distant, tests sur appareils et distribution |

Pour le prochain incrément, relever les modèles exacts des instruments, leurs interfaces (LAN SOCKET, VXI-11, HiSLIP, USB, GPIB), le runtime VISA déjà installé et un scénario de mesure prioritaire. Ces informations permettront de valider un driver réel sans supposer une compatibilité SCPI universelle.
