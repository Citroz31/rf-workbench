Windows x64 portable : télécharger **RF-Workbench-Windows-x64-portable.zip**, l'extraire entièrement puis lancer **rf-workbench.exe** à sa racine. L'EXE seul est aussi disponible dans les Assets. Le ZIP inclut onze projets exemples et le guide PDF de 39 pages.

### Version 0.11.0

- Recherche des composants dans les deux palettes, bouton d'effacement et Ctrl+F ; noms correspondants prioritaires, recherche floue en secours.
- Aimantation renforcée du câblage W : le curseur rejoint le pin compatible à l'entrée du rayon sous Windows, puis peut en sortir librement.
- Clic droit sur une légende ou une courbe pour modifier la mesure, le format et l'échelle, ou ajouter une trace dans la même fenêtre. Echelles indépendantes conservées.
- Panneau de résultats avec touches Instrument / Response / Stimulus / Utility et colonne de réglages. Freq, Power, Avg/BW, Channel, Meas, Format, Scale, Cal, Sweep et Trigger utilisent le projet et son plan PNA partagé.
- Petites fenêtres d'édition validées ; marqueur M1 local, recherche de maximum/minimum, Undo du panneau, Save/Recall et Preset paramètres S.

[Utilisation et captures](https://github.com/Citroz31/rf-workbench/blob/v0.11.0/docs/V0.11.md). La simulation fonctionne localement sans Python, VISA, compte ou Internet après téléchargement. Le manifeste Windows est **asInvoker**. Le pilotage réel nécessite les runtimes matériels appropriés ; le N5245B physique reste à qualifier sur le banc. Les marqueurs et recherches restent locaux aux données affichées.

Validation : tests Rust/Python, vrais événements souris egui, commande native de repositionnement du pointeur, conservation des échelles, validation des dialogues et des projets. Les checksums **SHA256SUMS.txt** couvrent l'EXE et le ZIP.
