Windows x64 portable : télécharger **RF-Workbench-Windows-x64-portable.zip**, l'extraire entièrement puis lancer **rf-workbench.exe** à sa racine. L'EXE seul est aussi disponible dans les Assets. Le ZIP inclut onze projets exemples et le guide PDF actualisé.

### Version 0.12.0

- Palette compacte et redimensionnable : catégories et recherche n’imposent plus leur largeur.
- Panneau VNA et graphes détachés sans agrandissement automatique, dimensions manuelles conservées, touches adaptées à la largeur disponible.
- Bloc PNA organisé en menus Instrument / Response / Stimulus / Utility, ouverture sur Quick Start.
- Choix des mesures, de 1 à 8 fenêtres et de leur affectation, aperçu du layout et superposition ; formats et échelles conservés.
- Fréquences, IFBW, puissance, sweep, trigger, calibration et chemins dans des sections séparées. Appliquer + configurer mémorise puis configure le même plan, sans sweep.
- Tests de réduction à la souris, stabilité pendant 60 images, interactions Quick Start et compatibilité des anciens projets.

[Utilisation et captures](https://github.com/Citroz31/rf-workbench/blob/v0.12.0/docs/V0.12.md). La simulation fonctionne localement sans Python, VISA, compte ou Internet après téléchargement. Le manifeste Windows est **asInvoker**. Un canal par bloc ; le pilotage réel nécessite les runtimes matériels appropriés et le N5245B physique reste à qualifier.

Validation : tests Rust/Python, vrais événements souris egui, commande native de repositionnement du pointeur, conservation des échelles, validation des dialogues et des projets. Les checksums **SHA256SUMS.txt** couvrent l'EXE et le ZIP.
