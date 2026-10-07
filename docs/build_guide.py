"""Build the French user guide from native captures and explicit prototype limits."""
from pathlib import Path
import math
import json
import numpy as np
from reportlab.graphics.shapes import Drawing, String, Line, Circle
from reportlab.graphics.charts.lineplots import LinePlot
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.lib import colors
from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
from reportlab.lib.enums import TA_LEFT
from reportlab.lib.pagesizes import A4
from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, PageBreak, Table, TableStyle, Image, KeepTogether, Flowable
from reportlab.pdfgen import canvas

HERE=Path(__file__).resolve().parent
WORK=HERE.parents[2]/'work'/'pdfs'
WORK.mkdir(parents=True,exist_ok=True)
FONT=Path('C:/Windows/Fonts/arial.ttf')
if not FONT.exists():
    import reportlab
    FONT=Path(reportlab.__file__).parent/'fonts'/'Vera.ttf'
BOLD=FONT.with_name('arialbd.ttf') if FONT.name=='arial.ttf' else FONT.with_name('VeraBd.ttf')
pdfmetrics.registerFont(TTFont('Guide',str(FONT)))
pdfmetrics.registerFont(TTFont('GuideBold',str(BOLD)))
pdfmetrics.registerFontFamily('Guide',normal='Guide',bold='GuideBold',italic='Guide',boldItalic='GuideBold')
TEAL=colors.HexColor('#007E86');INK=colors.HexColor('#152D42');PALE=colors.HexColor('#EAF5F5');GRAY=colors.HexColor('#526576')
styles=getSampleStyleSheet()
styles.add(ParagraphStyle('BodyGuide',fontName='Guide',fontSize=9.3,leading=14,spaceAfter=8,textColor=INK))
styles.add(ParagraphStyle('SmallGuide',fontName='Guide',fontSize=8.2,leading=12,spaceAfter=6,textColor=GRAY))
styles.add(ParagraphStyle('HGuide',fontName='GuideBold',fontSize=21,leading=27,spaceAfter=18,textColor=TEAL))
styles.add(ParagraphStyle('SubGuide',fontName='GuideBold',fontSize=12,leading=17,spaceBefore=10,spaceAfter=8,textColor=INK))
styles.add(ParagraphStyle('CoverGuide',fontName='GuideBold',fontSize=33,leading=40,spaceAfter=15,textColor=INK))
story=[]
def p(text,small=False):return Paragraph(text,styles['SmallGuide' if small else 'BodyGuide'])
def para(text,small=False):story.append(p(text,small))
def sub(text):story.append(Paragraph(text,styles['SubGuide']))
def page(number,title):
    if story:story.append(PageBreak())
    story.append(p(f'RF WORKBENCH 0.5  /  {number:02}',True));story.append(Paragraph(title,styles['HGuide']))
def steps(items):
    for i,item in enumerate(items,1):para(f'<b>{i:02}.</b> {item}')
def table(headers,rows,widths=None):
    data=[[p(x,True) for x in headers]]+[[p(str(x),True) for x in row] for row in rows]
    t=Table(data,colWidths=widths or [490/len(headers)]*len(headers),repeatRows=1,hAlign='LEFT')
    t.setStyle(TableStyle([('BACKGROUND',(0,0),(-1,0),PALE),('VALIGN',(0,0),(-1,-1),'TOP'),('LINEBELOW',(0,0),(-1,0),1,TEAL),('LINEBELOW',(0,1),(-1,-1),0.3,colors.HexColor('#CEDCDF')),('LEFTPADDING',(0,0),(-1,-1),7),('RIGHTPADDING',(0,0),(-1,-1),7),('TOPPADDING',(0,0),(-1,-1),7),('BOTTOMPADDING',(0,0),(-1,-1),6)]))
    story.append(t);story.append(Spacer(1,10))
def picture(name,caption,width=490):
    path=HERE/name
    if path.exists():
        from PIL import Image as PILImage
        with PILImage.open(path) as im:w,h=im.size
        story.append(Image(str(path),width=width,height=width*h/w));story.append(p(caption,True))
class Diagram(Flowable):
    def __init__(self):super().__init__();self.width=490;self.height=175
    def draw(self):
        c=self.canv;c.setFont('Guide',8)
        boxes=[(0,105,90,42,'PNA port 1'),(123,105,90,42,'PA 20 dB'),(246,105,90,42,'Coupleur 30 dB'),(382,105,100,42,'Charge RF')]
        for x,y,w,h,label in boxes:
            c.setFillColor(PALE);c.setStrokeColor(TEAL);c.roundRect(x,y,w,h,5,fill=1,stroke=1);c.setFillColor(INK);c.drawCentredString(x+w/2,y+17,label)
        c.setStrokeColor(TEAL)
        for a,b in [(90,123),(213,246),(336,382)]:c.line(a,126,b,126)
        c.line(291,105,291,66);c.line(291,66,382,66)
        c.setFillColor(PALE);c.roundRect(382,45,100,42,5,fill=1,stroke=1);c.setFillColor(INK);c.drawCentredString(432,62,'Pad 10 dB')
        c.line(432,45,432,22);c.line(432,22,300,22);c.drawString(155,19,'vers PNA port 2 / capteur')
        c.setFont('Guide',7.7);c.drawString(4,87,'Câble + plan de référence entrée DUT');c.drawString(248,157,'40 dBm = 10 W sur le trajet principal')
        c.drawString(10,2,'Schéma indicatif : capacités RF et calibration à vérifier pour 36-38 GHz.')

# Synthetic educational PA model; OP1dB exactly 34 dBm at Pin=15 dBm.
lo,hi=0.5,3.0
for _ in range(100):
    q=(lo+hi)/2
    loss=10/q*math.log10(1+10**(-5*q/10))
    if loss>1:lo=q
    else:hi=q
q=(lo+hi)/2
pin=np.linspace(-20,35,500);linear=pin+20
pout=linear-10/q*np.log10(1+10**(q*(linear-40)/10))
chart=Drawing(490,211)
lineplot=LinePlot();lineplot.x=47;lineplot.y=42;lineplot.width=422;lineplot.height=137
lineplot.data=[list(zip(pin.tolist(),linear.tolist())),list(zip(pin.tolist(),pout.tolist())),[(-20,40),(35,40)]]
lineplot.xValueAxis.valueMin=-20;lineplot.xValueAxis.valueMax=35;lineplot.xValueAxis.valueSteps=list(range(-20,36,10))
lineplot.yValueAxis.valueMin=0;lineplot.yValueAxis.valueMax=50;lineplot.yValueAxis.valueSteps=list(range(0,51,10))
for axis in [lineplot.xValueAxis,lineplot.yValueAxis]:
    axis.labels.fontName='Guide';axis.labels.fontSize=8;axis.gridStrokeColor=colors.HexColor('#DDE5E8');axis.visibleGrid=True
lineplot.lines[0].strokeColor=colors.HexColor('#8294A3');lineplot.lines[0].strokeDashArray=[4,3]
lineplot.lines[1].strokeColor=TEAL;lineplot.lines[1].strokeWidth=1.7
lineplot.lines[2].strokeColor=colors.HexColor('#D16B30');lineplot.lines[2].strokeDashArray=[1,3]
chart.add(lineplot)
mx=47+(15+20)/55*422;my=42+34/50*137
chart.add(Circle(mx,my,3,fillColor=colors.HexColor('#D16B30'),strokeColor=None))
chart.add(String(60,195,'OP1dB = 34 dBm ; Pin1dB = 15 dBm',fontName='GuideBold',fontSize=9,fillColor=TEAL))
chart.add(Line(285,190,mx,my,strokeColor=TEAL))
chart.add(String(185,17,'Pin au plan DUT (dBm)',fontName='Guide',fontSize=8))
chart.add(String(2,185,'Pout (dBm)',fontName='Guide',fontSize=8))
chart.add(String(50,3,'Pointillés gris : gain 20 dB ; teal : modèle synthétique ; orange : 40 dBm',fontName='Guide',fontSize=7.7))
csv=HERE.parent/'examples'/'PA-compression-synthetique.csv';csv.parent.mkdir(exist_ok=True)
csv.write_text('provenance,Pin_dBm,Pout_dBm,gain_dB\n'+''.join(f'simulation_pedagogique,{x:.4f},{y:.4f},{y-x:.4f}\n' for x,y in zip(pin,pout)),encoding='utf-8')

page(1,'Guide utilisateur')
story.append(Paragraph('RF Workbench',styles['CoverGuide']))
para('<b>Version 0.5 - Windows</b><br/>Bancs graphiques RF, DSP, scripts Python et instrumentation VISA/SCPI.')
story.append(Spacer(1,12));picture('PA.png','Application native : exemple petit signal PA 36-38 GHz, données simulées.')
sub('Du premier schéma à la lecture binaire du PNA-X')
para('Ce guide accompagne le prototype livré. Il décrit les actions disponibles, les unités, les limites et un protocole de caractérisation de PA : gain 20 dB, objectif 40 dBm, OP1dB voisin de 34 dBm, bande 36-38 GHz.')
para('Les résultats du banc PA fourni sont simulés. Aucun équipement physique n’a été connecté pour valider cette livraison. Les procédures haute puissance exigent les limites et calibrations du matériel effectivement utilisé.')
para('Édition du 7 octobre 2026. Sources : github.com/Citroz31/rf-workbench. Projet ouvert et extensible, sous licence MIT.')

page(2,'Parcours de lecture et périmètre')
table(['Sujet','Chapitres'],[['Installation, démarrage, projets','3-4'],['Confort, schéma, raccourcis','5-7'],['Connexion, PNA-X et options','8-11'],['Bibliothèque instruments','12-13'],['RF/DSP, mesures, unités','14-16'],['Python, fichiers et exécution','17-18'],['Exemple PA complet','19-22'],['Diagnostic et références','23-24']],[350,140])
sub('Fonctionnel dans cette livraison')
para('63 types de blocs, portage desktop Rust/egui, schéma typé, fenêtres de configuration, démarrage par projet, sauvegarde .rfbench, scripts Python, moteur DSP, acquisition binaire PNA et lecture FDATA des applications existantes autorisées. VISA dynamique, TCP/UDP et adaptateurs SDK complètent la HAL.')
sub('Frontières à connaître')
para('La validation locale concerne Windows et des serveurs de test. Un test de protocole ne valide ni le firmware d’un PNA réel, ni les options, ni la justesse métrologique. Les pilotes spécifiques AWG, thermique, capteurs et convertisseurs restent à écrire par modèle ; leurs simulations et console SCPI sont disponibles.')
para('Les applications GCA/NF/IMD/spectre se préparent sur l’instrument : cette version n’effectue pas leur configuration complète ni leur calibration automatique. Elle détecte les classes valides, récupère les données et propose un réglage de niveau GCA sur un canal existant.')
para('iOS/Android, synchronisation matérielle PTP/GPSDO/MIMO complète et streaming haut débit sans pertes ne sont pas livrés. Les nouveaux dialogues 0.5 sont en français ; la traduction anglaise des vues historiques reste disponible.')

page(3,'Installer et préparer le laboratoire')
steps(['Extraire <b>rf-workbench-windows.zip</b> dans un dossier accessible. Garder les fichiers du paquet ensemble. Lancer <b>Lancer RF Workbench.cmd</b> ou rf-workbench.exe. Aucun compilateur Rust n’est nécessaire.',
       'Pour les bancs simulés et la démonstration QAM, aucun runtime VISA ni SDK SDR n’est nécessaire. Le bloc Python exige Python 3.10 ou ultérieur, configuré dans l’onglet Python.',
       'Pour GPIB/USB/LAN VISA, installer NI-VISA, Keysight VISA ou le runtime compatible du laboratoire. Vérifier d’abord la ressource dans l’outil du constructeur, avec un processus de même architecture que l’application x64.',
       'Si nécessaire, définir RF_WORKBENCH_VISA vers la DLL/bibliothèque VISA choisie. Le chargement échoue explicitement si le runtime manque ou si l’architecture est incompatible.',
       'Pour un instrument LAN, vérifier son adresse, le routage et le protocole activé. VXI-11/HiSLIP utilisent VISA ; un socket SCPI brut utilise l’adresse TCPIP::hôte::port::SOCKET.'])
table(['Interface','Exemple de ressource'],[['GPIB','GPIB0::16::INSTR'],['USB','Ressource USB0::…::INSTR retournée par VISA'],['LAN VXI-11','TCPIP0::192.168.1.10::inst0::INSTR'],['LAN HiSLIP','TCPIP0::192.168.1.10::hislip0::INSTR'],['Socket SCPI','TCPIP::192.168.1.10::5025::SOCKET'],['Série via VISA','ASRL5::INSTR'],['Simulation','SIM::RF::INSTR']],[155,335])
para('Ne pas transformer une adresse INSTR en SOCKET : ce sont des protocoles différents. Les réglages série restent dans le runtime/profil constructeur.',True)

page(4,'Créer, ouvrir et sauvegarder un projet')
picture('START.png','Écran de démarrage. Les quatre exemples sont accessibles sans instrument.')
steps(['Saisir le nom et cliquer <b>Nouveau projet vide</b>, ou choisir un exemple. Le banc courant reste accessible dans les espaces de travail de la session.',
       'Cliquer <b>Ouvrir un projet</b>. Parcourir les dossiers, sélectionner un .rfbench ou saisir son chemin complet. Les fichiers .rfw.json historiques sont acceptés.',
       'Utiliser l’icône Enregistrer ou Ctrl+S : choisir le chemin puis Enregistrer. L’extension .rfbench est ajoutée/normalisée. Les réglages, ressources et layout sont inclus ; les mesures acquises ne le sont pas.',
       'À chaque ouverture, le mode Matériel est désactivé. Vérifier les adresses puis l’activer explicitement pour une acquisition physique.'])
para('<b>.rfbench</b> est une enveloppe JSON versionnée : format rf-workbench/bench, version 1, projet et layout. Limite 4 Mo / 1000 blocs. Un futur format non reconnu est refusé. Pour conserver plusieurs bancs et préférences de session, utiliser Enregistrer Studio (.rfw.json distinct).')
para('L’application accepte --open chemin.rfbench ou un chemin .rfbench comme argument. L’association Windows à l’extension n’est pas installée automatiquement.',True)

page(5,'Adapter l’interface à la souris')
steps(['Placer le pointeur sur la frontière de la palette, de l’inspecteur ou du journal. Glisser le séparateur pour augmenter/réduire la taille. La zone de prise est élargie pour faciliter la manipulation.',
       'Les vues dockées à droite ou en bas se redimensionnent par leur séparateur. Disposition permet de changer la vue principale, docker, masquer ou faire flotter une analyse.',
       'Une fenêtre flottante se déplace par sa barre de titre et se redimensionne par son bord ou coin inférieur. Les fenêtres de configuration d’instrument ont un contenu défilant pour garder les boutons accessibles.',
       'Les graphiques cartésiens disposent d’un coin de redimensionnement : glisser pour changer leur hauteur indépendamment. Leur hauteur peut varier de 100 à 900 pixels. Les vues spécialisées restent pilotées par leur panneau parent.',
       'Enregistrer le projet ou un layout conserve les dimensions des panneaux principaux, docks et fenêtres d’analyse. Les hauteurs locales des graphiques sont conservées en session, pas dans .rfbench.'])
table(['Élément','Plage indicative'],[['Palette','150-600 px'],['Inspecteur','220-850 px'],['Journal','45-600 px'],['Dock droit / bas','240-900 px / 130-700 px'],['Configuration instrument','Fenêtre ajustable, minimum 470 × 340 px']],[200,290])
sub('Retrouver une disposition confortable')
para('Espaces de travail propose un layout banc + spectre, un layout debug, des layouts nommés, le thème clair, le contraste et une échelle de texte. Si l’écran devient chargé, masquer l’inspecteur ou déplacer l’analyse dans une fenêtre flottante. Les limites minimales évitent de rendre les commandes inaccessibles.')

page(6,'Construire et câbler un schéma')
picture('DSP-GRAPH.png','Exemple de graphe : les câbles représentent des dépendances de données, pas un câblage RF physique.')
steps(['Dans la palette ou Blocs, rechercher un type et cliquer pour l’ajouter. Déplacer le bloc par glisser-déposer. La grille et l’alignement facilitent la construction.',
       'Sélectionner un bloc. L’inspecteur présente ses ports et ses commandes rapides ; Configurer l’instrument ouvre les fonctions détaillées. F2 affiche son aide.',
       'Appuyer sur <b>W</b>, cliquer un port de sortie puis une entrée compatible. Cliquer le fond entre les deux pour définir un coude. Échap annule ; clic droit sur un câble le retire.',
       'Les ports RF, Trace, réseau complexe, I/Q, bits et unités sont distincts. Une entrée requise doit être câblée. Les cycles et entrées déjà reliées sont refusés.',
       'Utiliser multi-sélection, copier/coller, annotations et commentaires. Un schéma peut être sauvegardé incomplet ; Exécuter exige un graphe valide.'])
para('Un câble graphique ne commande aucun commutateur RF et ne change aucune connexion de laboratoire. Les ressources des blocs et le montage physique doivent correspondre au banc dessiné.',True)

page(7,'Palette, navigation et raccourcis')
para('La palette regroupe instruments RF, conversion, thermique, DUT, automatisation, sources/HAL, DSP/canal, modulation, codage, mesures et calibration. La recherche est floue ; un clic droit ajoute un favori. Les blocs récents restent accessibles.')
table(['Action','Raccourci par défaut'],[['Câbler / déplacer le canevas','W / H'],['Exécuter / supprimer','F5 / Suppr'],['Annuler / rétablir','Ctrl+Z / Ctrl+Y'],['Copier / coller','Ctrl+C / Ctrl+V'],['Enregistrer / ouvrir','Ctrl+S / Ctrl+O'],['Debug / pas / continuer','F6 / F10 / F8'],['Aide globale / bloc','F1 / F2'],['Préférences raccourcis','Ctrl+K'],['Navigation clavier du graphe','Tab, flèches et Entrée']],[290,200])
para('Raccourcis accepte plusieurs combinaisons par action, séparées par des virgules. Enregistrer les préférences les applique ; les conflits sont signalés. Sur macOS, Ctrl correspond à Cmd pour les actions usuelles.')
sub('Éviter une action involontaire pendant la saisie')
para('Les raccourcis d’édition sont suspendus quand un champ de texte a le focus, ou pendant les nouvelles fenêtres projet/instrument. Utiliser les boutons du dialogue pour confirmer les réglages. Les boutons Appliquer mémorisent le bloc ; ils ne déclenchent pas une mesure, sauf quand un bouton annonce explicitement une action SCPI.')
sub('Aide intégrée')
para('F1 ouvre l’aide globale ; F2 ou Aide du bloc explique ports, exemples, formules et limites. Les tests de l’onglet Tests vérifient le moteur avec des données de référence simulées, sans qualifier un équipement réel.')

page(8,'Identifier et sélectionner un équipement')
steps(['Sélectionner un bloc instrument, puis <b>Configurer l’instrument</b>. La fenêtre comprend Connexion, Fonctions, Options PNA et Console SCPI.',
       'Dans Connexion, choisir un timeout. Activer Matériel puis Détecter VISA + identifier. La découverte VISA liste les ressources accessibles ; chaque candidate est interrogée avec *IDN?. Stop interrompt entre les tentatives.',
       'Pour un PNA, le pilote reconnaît les familles Keysight/Agilent N52xx/E83xx. Pour les autres appareils, saisir éventuellement un fragment IDN attendu : modèle ou numéro de série.',
       'Si une seule candidate correspond et si le bloc est encore simulé, l’adresse est proposée dans le brouillon. En cas de plusieurs réponses, sélectionner dans la liste. Une adresse déjà choisie manuellement n’est pas remplacée.',
       'Si le LAN n’est pas découvert, saisir son adresse VISA ou socket. Aucune exploration automatique de sous-réseau n’est effectuée. La candidate manuelle est aussi identifiée.',
       'Cliquer Appliquer au bloc. Les valeurs persistent dans .rfbench. Fermer sans appliquer laisse la configuration précédente.'])
para('Les ressources USB nécessitent le support du constructeur ; un capteur USB propriétaire n’est pas automatiquement USBTMC. Les appareils qui ne répondent pas à *IDN? exigent une sélection manuelle et un profil adapté. Les pilotes Soapy/UHD/IIO utilisent les blocs HAL, avec leurs chaînes de ressources propres.')
para('La découverte est plafonnée à 64 candidates et chaque tentative possède son timeout. Un timeout ne démontre pas qu’un équipement est absent : adresse, runtime, firmware ou session exclusive peuvent empêcher sa réponse.')

page(9,'PNA-X : canaux et transfert rapide')
picture('INSTRUMENT.png','Fonctions PNA : Channel, mesure, format binaire et contrôle explicite du balayage.')
steps(['Dans Connexion, Lire canaux / options PNA. Vérifier l’identité et les canaux présents. Dans Fonctions, sélectionner le Channel par numéro ou dans la liste retournée.',
       'Relire les options après avoir changé de canal. Choisir une mesure existante du catalogue. Si le nom reste vide, la lecture SDATA exige une seule mesure correspondant au paramètre S choisi.',
       'Choisir REAL32 pour les données complexes compactes ou REAL64. L’axe est toujours acquis en REAL64 sur l’équipement pour éviter de reconstruire un axe arrondi.',
       'Appliquer au bloc et Exécuter. Le port réseau fournit magnitude/phase de SDATA, avec le paramètre vérifié dans le catalogue. La vue Paramètres S / Smith exploite ce résultat.'])
para('Le port réseau du bloc exige un canal Standard et un balayage de fréquence croissant. Les canaux de puissance ou d’application se lisent par FDATA dans la fenêtre dédiée. Une mesure ambiguë, une taille incohérente ou NaN/Inf est refusée.',True)

page(10,'Choisir ce qui change sur le PNA')
table(['Commande utilisateur','Comportement'],[['Aucune case cochée','Lire les dernières données calculées, sans nouvelle acquisition.'],['Appliquer le balayage','Configurer début/fin, points, IFBW et moyennage avant lecture.'],['Déclencher un nouveau balayage','INIT&lt;canal&gt;:IMM puis attente *OPC? ; nécessite le réglage de trigger compatible sur l’appareil.'],['Exécuter en continu','Répéter le graphe ; la session PNA est réutilisée tant que ressource et timeout restent identiques.']],[200,290])
para('Un fichier projet n’est pas une calibration. La lecture ne fait pas de preset, ne crée pas de trace, ne choisit pas automatiquement de cal set et n’active/désactive pas la puissance RF du PNA. Le PNA conserve son état RF. Vérifier ses modes de trigger et de sweep avant une acquisition déclenchée.')
sub('Ce qui est écrit pendant la lecture')
para('Le pilote sélectionne la mesure demandée avec CALC&lt;ch&gt;:PAR:SEL. Il sauvegarde FORM:DATA et FORM:BORD, impose le byte order little endian, lit l’axe puis SDATA, et restaure le format de transfert. Une erreur de restauration est signalée. La sélection de trace reste celle demandée.')
para('Une acquisition binaire évite l’analyse d’une grande chaîne de nombres ASCII. Le débit réel dépend du bus, du nombre de points, des calculs VNA et de l’IFBW ; aucun benchmark de matériel réel n’a été réalisé. REAL32 réduit le payload SDATA de moitié par rapport à REAL64. Pour N points : axe 8N octets, SDATA 8N ou 16N octets, hors en-têtes et protocole.')
sub('Mesure ou données déjà disponibles ?')
para('Exécuter avec les cases désactivées n’implique pas que les données sont fraîches. Enregistrer avec les résultats le canal, le nom de mesure, le numéro de série, le temps de mesure, les conditions RF et la calibration utilisée. Les métadonnées d’une simple NetworkTrace ne constituent pas à elles seules un journal métrologique complet.')

page(11,'Options et applications PNA-X')
table(['Source de détection','Usage'],[['*IDN?','Identifier le modèle et le numéro de série.'],['*OPT?','Afficher les options déclarées.'],['SYST:CAP:LIC:CAT? VALID','Afficher les licences/options valides si cette commande existe.'],['SYST:MCL:VAL:CAT?','Autoriser les classes réellement valides ; ne pas utiliser le catalogue général pour débloquer une fonction.'],['SYST:CHAN:CAT? / SENS&lt;ch&gt;:CLAS:NAME?','Canaux existants et classe du canal choisi.']],[235,255])
para('Options PNA affiche Standard, Gain Compression, Noise Figure Cold Source, Spectrum Analyzer, Swept IMD et IM Spectrum. Inconnu reste désactivé. La présence d’un modèle ou d’un code 086/029/090 ne remplace pas une classe valide rapportée par l’appareil. Un firmware ancien peut rendre la capacité indéterminée.')
steps(['Préparer le canal d’application et sa calibration sur le PNA. Relire les capacités et choisir le nom de mesure dans Fonctions.',
       'Options PNA → Lire FDATA binaire du canal/application. Le pilote revérifie la classe valide et la trace. La lecture du format scalaire conserve l’axe et les valeurs d’affichage de l’instrument ; elle ne déclenche pas de sweep.',
       'Consulter le format, le nombre de points et les courbes. Exporter JSON vers un fichier neuf ; nommer l’extension .json. Pour un format Polar/Smith à deux valeurs par point, cette voie scalaire est refusée.',
       'Sur un canal Gain Compression existant autorisé, choisir le niveau puis Appliquer niveau GCA. Le pilote revérifie la classe avant d’envoyer le réglage. Les autres paramètres avancés se configurent sur l’équipement ou via la console.'])
para('L’option Spectrum Analyzer peut avoir une plage en fréquence différente de celle du VNA. À 36-38 GHz, vérifier la licence, la bande, les sources/récepteurs et les atténuateurs ; le bouton Disponible n’atteste aucune de ces conditions.',True)

page(12,'Fonctions des blocs instruments RF')
table(['Bloc','Réglages et chemin effectif'],[['Générateur RF','Fréquence et puissance. Simulateur ou SCPI générique FREQ/POW/OUTP ; arrêt demandé en fin/Stop.'],['Analyseur de spectre','Début/fin/points, requête ASCII configurable. Profil générique et axe configuré localement à valider sur le modèle.'],['PNA / PNA-X','Canal, trace, S11/S21/S12/S22, REAL32/64, IFBW, moyennage, déclenchement optionnel, SDATA native.'],['AWG','Cadence, tone, nombre d’échantillons, résolution, pleine échelle ; formes d’onde simulées. Console réelle ; téléchargement constructeur à développer.'],['Noise Figure Meter','Facteur de bruit simulé / modèle DUT. Connexion et console ; profil NF constructeur à développer.'],['Power Sensor / Power Meter','Chaîne RF → sensor → meter simulée, unité dBm. Console et commandes de profil pour un appareil réel ; lecture typée disponible dans la bibliothèque HAL.']],[145,345])
para('Chaque bloc éligible possède connexion, timeout, identification attendue et commandes de profil. Les commandes Lecture et Consigne du profil sont des raccourcis de console, pas des actions automatiques du graphe. Consulter le manuel avant un write.')
sub('Console commune')
para('Query texte attend une réponse ; Envoyer write ne la lit pas. Choisir une commande correspondant au mode. Query binaire → fichier extrait le payload d’un bloc IEEE 488.2 à longueur définie, sans convertir son type numérique ; la limite de réponse est 8 Mio. Le fichier doit être neuf. Nommer le fichier .bin et noter précision, endian et unité séparément.')
para('Le bouton Erreurs lit SYST:ERR? : cette interrogation peut retirer une entrée de la file d’erreurs. Les commandes utilisateur peuvent activer RF ou changer les calibrations. Elles ne sont ni filtrées en dialecte constructeur ni relancées automatiquement.')

page(13,'Conversion, thermique et DUT')
table(['Bloc','Usage et limite'],[['DAC / CAN','Cadence, tone, échantillons, résolution, pleine échelle. Modèle de conversion borné/quantifié ; aucune cadence matérielle inventée.'],['Modulateur I/Q','Relier I, Q et LO ; même taille/cadence/tone pour I et Q. Perte de conversion réglable, modèle idéal simulé.'],['Résistance variable','Résistance locale en Ω ; identification/console pour instrument réel.'],['Thermomètre','Entrée TEMP ou valeur locale en °C. Lecture du profil à adapter au modèle.'],['Thermostream','Consigne idéale en °C ; aucune dynamique de stabilisation ni pilote constructeur implicite.'],['DUT','Modèle RF avec gain/perte, NF et lien catalogue. Perte négative = gain ; aucune compression non linéaire dans ce modèle.'],['Catalogue DUT','Ajout manuel, recherche, JSON import/export. Base initiale vide ; pas de scraping des fabricants dans cette livraison.']],[150,340])
sub('Choisir une représentation juste')
para('Un DUT graphique décrit les données de modèle utilisées par le PNA/NF simulé. Il n’alimente pas une puce ni ne vérifie ses limites de puissance. Les connexions et métadonnées restent des descriptions de banc. Ajouter une annotation pour la polarisation, les plans de référence, les pertes et les protections.')
para('Pour un appareil non encore piloté par le graphe, la console fournit un accès SCPI explicite. Un pilote spécifique devra fixer format, unités, commandes, synchronisation et comportement d’arrêt. Les blocs simulés refusent une ressource physique tant qu’un pilote réel n’existe pas.')

page(14,'Utiliser le laboratoire RF / DSP')
picture('DSP.png','Banc QAM16 / AWGN / Viterbi : I/Q et constellation proviennent du moteur exécuté.')
steps(['RF / DSP → Charger QAM16 / AWGN / Viterbi. Exécuter ou F5. Le graphe code 1024 bits, module, ajoute du bruit, démodule et décode.',
       'Choisir les buffers I/Q et Spectrum dans les listes. Consulter constellation, EVM, SNR, BER et puissance. À SNR demandé 30 dB, l’exemple de référence donne BER 0, EVM environ 3,18 % et SNR environ 29,95 dB.',
       'Spectre & waterfall montre la PSD et l’historique. Continu ajoute les acquisitions. Le seed rend la simulation reproductible ; BER 0 sur 1024 bits ne prouve pas une BER nulle.',
       'Configurer les blocs dans l’inspecteur, avec réglages courants et JSON avancé validé. Les aides F2 précisent les profils et ports.'])
para('Les signaux générés peuvent être en V RMS ou FS. Pour SDR/audio, utiliser FS avec marge de crête, par exemple amplitude 0,5 en QAM16. Les sorties refusent |I+jQ| &gt; 1 ; une mesure de puissance physique exige une conversion calibrée FS → V.',True)

page(15,'DSP : filtres, modems et codes')
table(['Famille','Réglages et limites à retenir'],[['FIR / IIR','Taps et coefficients, état entre trames. Concevoir la stabilité et l’anti-alias selon le signal.'],['Décimation / interpolation','Facteur entier, FIR configurable ; cadence modifiée. Pas de resampling fractionnaire universel.'],['FFT / Welch / fenêtres','Radix 2 ; Hann, Hamming, Blackman, rectangulaire ; Welch 50 % overlap.'],['AGC / PLL / Costas','Gain automatique, boucles second ordre ; Costas BPSK/QPSK.'],['Timing / frame sync','Gardner par trame ; préambule exact/inversé. Synchronisation fractionnaire continue et préambule entre trames non complets.'],['AM / FM / PM','Modulation/démodulation analogique ; vérifier niveaux et bande.'],['ASK / FSK / PSK / QAM','Timing connu, pulses rectangulaires ; QAM 4/16/64/256. LLR max-log ; confiance fixe après décision FSK.'],['OFDM','FFT N, CP N/8 et QAM tous bins. Ni pilotes ni égaliseur ni norme Wi-Fi/5G implicite.'],['Convolutionnel / RS','K=3 (7,5), rate 1/2, Viterbi ; RS GF(256), parité 2..64 octets.'],['LDPC / Turbo','Profils de recherche propres au projet, pas 3GPP/DVB. Voir V0.4.md pour matrices/interleaver et limites.'],['Canal RF','AWGN seedé, Rayleigh constant par trame, multipath FIR, rotation Doppler et compression sans mémoire.']],[170,320])
para('Les unités, cadence, fréquence centrale et timestamps font partie des données. Une discontinuité ou un changement de paramètres réinitialise les historiques de filtre/boucle. Ne pas assimiler ces profils de recherche à une chaîne radio normalisée qualifiée.')

page(16,'Mesures, calibration et unités')
table(['Mesure','Définition / condition'],[['Puissance complexe','P = mean(|I+jQ|²) / R, avec amplitude complexe définie en V RMS.'],['SNR / EVM','10 log10(signal/error) ; 100 × sqrt(error/signal). Référence de même taille, cadence, fc, unité et timestamps.'],['BER / PER','Bits/paquets comparés à une référence explicite ; conserver le nombre total et les erreurs.'],['THD','Harmoniques 2..10, référence cohérente connue.'],['Bruit de phase','Phase déroulée, tendance retirée, PSD de fluctuations ; approximation faible phase, sans cross-correlation.'],['VSWR','(1+|Γ|)/(1-|Γ|), sur S11/S22 ; refus de S21 et |Γ| ≥ 1.'],['PSD','|FFT(xw)|² / (Fs × sum(w²)), deux côtés ; V²/Hz ou FS²/Hz. Pas de dBm implicite.']],[150,340])
sub('Calibration logicielle')
para('Les blocs permettent offset DC, matrice de gain/phase/DC I/Q, correction de gain et phase porteuse du câble, table [Hz,dB,deg] à fc, et volts_per_fs. Ils ne remplacent pas la calibration VNA ni une correction de retard d’enveloppe complète. Pas d’extrapolation de table.')
para('Unités : Hz, dBm, dB, dBV/dBFS, V, A, W, s, sample/s, ratios, pourcentages et densités. 0 dBm = 1 mW ; sur 50 Ω, Vrms = 223,6068 mV. Les conversions dimensionnellement impossibles sont refusées.')
para('Incertitude : U = k × sqrt(sum(u²)) pour composantes indépendantes dans une même unité. Corrélations, covariance, degrés de liberté et propagation dans le graphe ne sont pas calculés automatiquement. Le budget du laboratoire reste nécessaire.')

page(17,'Python, automatisation et exécution')
steps(['Dans Python, choisir l’interpréteur et tester la connexion. Charger un exemple ou écrire un script local.',
       'Un bloc Python reçoit trace et produit output. ResourceManager peut ouvrir une ressource et effectuer write/query via la passerelle. Appliquer le script au bloc sélectionné avant d’exécuter le graphe.',
       'Inspecter le journal : exceptions, erreurs de forme, cancellation et timeout apparaissent. Stop termine un script bloqué. Les scripts locaux disposent des droits du compte utilisateur.',
       'En debug simulé : F6 démarre, F10 exécute un bloc, F8 poursuit ; breakpoints et sondes sont dans l’inspecteur. Les buffers live montrent leurs métadonnées et une prévisualisation bornée.'])
sub('Exemple de compensation de câble')
para('<font face="Guide">output = dict(trace)<br/>output[\'amplitude_dbm\'] = [x + 0.5 for x in trace[\'amplitude_dbm\']]</font>')
para('Cette correction uniforme est pédagogique. En métrologie, utiliser les pertes mesurées à chaque fréquence et les incertitudes correspondantes. Ne pas appliquer deux fois la même compensation entre Python, calibration instrument et table logicielle.')
sub('Exécution et arrêt')
para('Les calculs et I/O s’effectuent hors du thread de rendu. Le dernier résultat remplace le précédent. Le mode Continu ajoute une pause d’environ 250 ms entre cycles de graphe ; ce n’est pas un ordonnanceur SDR haut débit. Les sorties génériques du générateur sont arrêtées au mieux en fin/Stop. La console et le PNA conservent l’état commandé : vérifier RF sur l’appareil.')
para('Les limites de réponse, de projet et de prévisualisation évitent de saturer l’interface. La preview ne contient pas forcément tous les échantillons du buffer original. Les autotests intégrés sont accessibles dans Tests ; ils n’effectuent pas de qualification matérielle.')

page(18,'Acquisition, stockage et rejeu')
table(['Backend HAL','Dépendance / contrat'],[['RAW','Rust natif, cf32 little endian + sidecar JSON versionné et index ; lecture/seek/rejeu.'],['TCP / UDP','Trames I/Q JSON ; TCP longueur u32 big endian, UDP un datagramme borné. Distinct du socket SCPI.'],['VISA I/Q','Bloc cf32 LE générique, format et query à préparer sur l’instrument.'],['Soapy / UHD / IIO','SDK Python constructeur + numpy ; pilotes mono-canal. AD9361 pour IIO, échelles à vérifier.'],['Audio / ZMQ','sounddevice/PortAudio ; pyzmq. Audio stéréo I/Q, ZMQ SUB/PUSH.'],['HDF5 / Parquet','h5py / pyarrow + numpy ; schémas du projet, métadonnées par trame, fichier neuf.']],[145,345])
para('Chaque IqFrame transporte Fs, fc, unité, provenance, indice, epoch optionnelle et domaine d’horloge. L’epoch n’est pas automatiquement UTC. L’acquisition réseau/SDR utilise une queue SPSC de capacité 4 ; une trame nouvelle est abandonnée si la queue est pleine, avec compteur et discontinuité. Le journal signale les pertes.')
para('Les adaptateurs Python restent supervisés par timeout ; un SDK bloqué est terminé. Un arrêt forcé peut laisser un fichier Parquet incomplet. RAW peut écraser explicitement une sortie lors d’une nouvelle exécution : choisir un chemin neuf pour une campagne à conserver.')
para('Installer seulement les backends nécessaires. Pour stockage/réseau optionnels : python -m pip install numpy h5py pyarrow pyzmq. Les SDK SDR viennent de leurs distributions constructeur. Un SDK manquant produit une erreur ; aucune donnée simulée n’est présentée comme physique.')

page(19,'Exemple PA : définir les objectifs')
table(['Grandeur demandée','Interprétation dans cet exemple'],[['Bande','36-38 GHz ; points de contrôle 36, 37 et 38 GHz.'],['Gain petit signal','20 dB, à vérifier par fréquence.'],['P1dB','Hypothèse : OP1dB ≈ 34 dBm, puissance de sortie.'],['Pin au point de compression','Gain comprimé = 19 dB ; Pin1dB ≈ 34 - 19 = 15 dBm.'],['Puissance maximale','40 dBm = 10 W, objectif séparé ; le point d’entrée nécessaire ne se déduit pas du gain petit signal.']],[185,305])
para('Si « P1dB 34 dBm » désigne au contraire une puissance d’entrée, l’interprétation change complètement. Pour ce guide, OP1dB désigne explicitement la sortie. 34 dBm correspond à environ 2,512 W ; 15 dBm à 31,62 mW à l’entrée.')
story.append(chart);story.append(p('Courbe pédagogique synthétique, pas une mesure du PA. L’asymptote 40 dBm illustre une saturation choisie.',True))
para('Le CSV PA-compression-synthetique.csv accompagne le guide. Le fichier PA-36-38GHz.rfbench exécute uniquement le petit signal : gain nominal 20 dB avec ripple illustratif de ±0,15 dB. Il ne simule ni P1dB, ni échauffement, ni Psat ; ces valeurs sont des objectifs annotés.')

page(20,'Exemple PA : préparer le montage')
story.append(Diagram());story.append(Spacer(1,12))
steps(['Vérifier la fiche PA : polarisation, courant, température, puissance d’entrée maximale, ordre d’alimentation et duty cycle. Choisir des câbles/connecteurs, coupleur, pads et charge spécifiés jusqu’à 38 GHz, avec marge thermique.',
       'Le trajet principal peut transporter 10 W CW. Prévoir une charge/coupleur dont les limites réelles dépassent cette puissance avec la marge requise. Le port VNA ou capteur ne reçoit jamais directement la sortie 40 dBm.',
       'Exemple indicatif : coupleur 30 dB, puis pad 10 dB sur la branche de mesure. À 40 dBm, le niveau nominal sur cette branche est 0 dBm hors pertes. Ce niveau doit encore être comparé aux limites de compression/dommage du récepteur réel.',
       'Établir les plans de référence DUT et calibrer le trajet source ainsi que la branche récepteur. Mesurer les pertes, couplage, mismatch et directivité sur la bande ; leur somme nominale de 40 dB ne suffit pas pour une calibration métrologique.',
       'Vérifier qu’un PNA/source couvre 36-38 GHz et fournit le niveau d’entrée nécessaire après les pertes. Un booster éventuel exige sa propre calibration et ses limites. Mesurer la température/courant et définir les conditions d’arrêt avant la montée en puissance.'])
para('Le dessin est un exemple de principe, pas une prescription de composants. Le modèle du PNA-X, ses récepteurs/atténuateurs et leurs limites ne sont pas connus dans cette session. Les valeurs constructeur et les contrôles RF physiques priment.',True)

page(21,'Exemple PA : campagne de mesure')
sub('A. Petit signal, puis référence de gain')
steps(['Ouvrir l’exemple PA dans Accueil. Exécuter en simulation pour vérifier la bande et les vues. Puis créer une copie .rfbench pour le matériel et remplacer la ressource PNA.',
       'Préparer le canal Standard et sa trace S21 sur l’équipement. Vérifier les plans de calibration. Dans RF Workbench : Channel, trace, S21, REAL32/64, lecture du canal existant ; relire les options puis Appliquer.',
       'Au laboratoire, démarrer au niveau faible compatible avec le PA et le bruit de mesure, par exemple un point de départ -30 dBm au plan d’entrée après validation. Mesurer le gain à 36/37/38 GHz et conserver Gref(f).'])
sub('B. Mesurer OP1dB')
steps(['Sur un canal GCA autorisé : préparer la source power cal et la receiver power cal dans le PNA, choisir la référence de gain et les limites de puissance. Relire la classe/trace ; régler une compression de 1 dB depuis la fenêtre si souhaité.',
       'Exécuter le balayage sur l’instrument, puis lire FDATA. Pour chaque fréquence, enregistrer Pin, Pout, gain et conditions. Si Gref=20 dB et OP1dB=34 dBm, vérifier G=19 dB et Pin≈15 dBm.',
       'Sans GCA : à fréquence fixe, effectuer un power sweep contrôlé sur l’équipement ou une boucle de mesure qualifiée. Calculer G(Pin)=Pout-Pin et chercher Gref-1 dB, avec interpolation entre les points qui encadrent le seuil. Répéter par fréquence.'])
sub('C. Approcher les 40 dBm')
para('Étape séparée, uniquement dans les limites validées de Pin, température, courant et charge. Utiliser un capteur/receiver calibré et des pas adaptés près de la saturation. La puissance d’entrée requise ne vaut pas automatiquement 20 dBm : le gain est déjà comprimé. Le prototype n’automatise pas cette montée haute puissance ni ses interlocks.')

page(22,'Exemple PA : exploiter les résultats')
table(['Fréquence','Gref cible','Pin1dB estimé','OP1dB cible'],[['36 GHz','20 dB','15 dBm','34 dBm'],['37 GHz','20 dB','15 dBm','34 dBm'],['38 GHz','20 dB','15 dBm','34 dBm']],[110,110,130,140])
para('Ce tableau est un objectif théorique, pas un relevé. Remplacer chaque case par le résultat mesuré et son incertitude. Si le gain réel Gref varie, Pin1dB=OP1dB-(Gref-1), fréquence par fréquence.')
sub('Interpréter le trajet de réception')
para('Exemple nominal : couplage 30 dB + pad 10 dB donnent 40 dB de perte vers le récepteur. Une sortie 34 dBm devient -6 dBm ; une sortie 40 dBm devient 0 dBm, hors pertes additionnelles. Corriger avec la calibration réelle dépendante de f, pas seulement une addition constante. Le mismatch et la directivité limitent la justesse.')
sub('Conserver une trace exploitable')
para('Archiver : .rfbench, JSON FDATA / payload binaire avec précision-endian-unités, identités et licences, paramètres de sweep, cal sets et dates, plans de référence, température et courant, Pin/Pout par fréquence, tables de pertes et incertitudes. Un payload .bin seul est insuffisant pour reconstituer les mesures.')
sub('Budget d’incertitude')
para('Inclure les contributions de source power cal, receiver/sensor cal, coupleur/pad/câbles, mismatch, directivité, dérive et répétabilité. Évaluer leurs corrélations avant combinaison. Pour P1dB, inclure le pas de puissance et l’incertitude de l’interpolation ; vérifier le seuil avec une référence de gain stable.')
para('À fort échauffement, le gain et P1dB peuvent dépendre du temps de stabilisation et du duty cycle. Signaler les conditions plutôt que comparer directement une mesure CW chaude à une valeur constructeur obtenue en impulsions.')

page(23,'Diagnostic et limites pratiques')
table(['Symptôme','Vérifications'],[['VISA absent','Runtime x64 installé, choix de DLL, test constructeur, ressource INSTR correcte.'],['Adresse non listée','Appareil allumé, driver GPIB/USB, interface LAN activée ; saisir adresse et identifier manuellement.'],['Plusieurs appareils correspondants','Filtrer par numéro de série IDN attendu ou sélectionner explicitement.'],['Options désactivées','Relire pour la bonne ressource et le bon canal ; capacité VALID inconnue ou licence manquante.'],['Mesure PNA ambiguë','Sélectionner un nom exact du catalogue, vérifier S-parameter et classe Standard.'],['Erreur SDATA / FDATA','Format scalaire vs complexe, longueur et axe ; vérifier mesure calculée et absence de NaN.'],['Timeout *OPC?','Trigger source / sweep mode, IFBW, moyennage et durée ; augmenter le délai dans ses bornes.'],['Erreur de restauration','Reconnecter, vérifier FORM:DATA/FORM:BORD sur l’équipement ; ne pas ignorer l’état incertain.'],['Overflow HAL','Réduire débit/charge, vérifier taille de trame ; le graphe 250 ms et JSON ne sont pas adaptés au streaming SDR maximal.'],['Fenêtre trop grande','Glisser les bords/coins, défiler le contenu, réduire l’échelle de texte ou revenir à un layout de base.']],[175,315])
para('Les scripts, fichiers RAW et consoles peuvent modifier/écraser des données selon leurs modes. Sauvegarder les mesures dans un dossier de campagne neuf. Le Stop logiciel n’est pas un interlock matériel.')
para('Les tests de livraison vérifient les graphes, le binaire IEEE 488.2 fragmenté, les unités, la restauration et le refus de classes non autorisées. Les détails de version et CI sont dans docs/VALIDATION.md et GitHub Actions. Aucun de ces contrôles n’est une calibration ou certification métrologique.')

page(24,'Références et évolutions')
refs=[('Code, versions et CI','https://github.com/Citroz31/rf-workbench'),('Keysight : système, canaux et classes VALID','https://helpfiles.keysight.com/csg/NA520xA/Programming/GP-IB_Command_Finder/System.htm'),('Keysight : licences et capacités','https://helpfiles.keysight.com/csg/NA520xA/Programming/GP-IB_Command_Finder/SystCapability.htm'),('Keysight : SDATA / FDATA','https://helpfiles.keysight.com/csg/NA520xA/Programming/GP-IB_Command_Finder/Calculate/Data.htm'),('Keysight : axes instrument','https://helpfiles.keysight.com/csg/NA520xA/Programming/GP-IB_Command_Finder/Calculate/X_Values.htm'),('Keysight : classe du canal','https://helpfiles.keysight.com/csg/NA520xA/Programming/GP-IB_Command_Finder/Sense/Class.htm'),('Keysight : réglages GCA','https://helpfiles.keysight.com/csg/NA520xA/Programming/GP-IB_Command_Finder/Sense/Gain_Compression.htm'),('Keysight : concepts de compression','https://helpfiles.keysight.com/csg/e5080b/Applications/Gain_Compression_Application.htm'),('UHD : synchronisation','https://files.ettus.com/manual/page_sync.html'),('liquid-dsp : familles DSP','https://www.liquidsdr.org/doc/index.html')]
for label,url in refs:
    para(f'<b>{label}</b><br/><link href="{url}" color="#007E86">{url}</link>')
sub('Documents du dépôt')
para('V0.5.md décrit la nouvelle interface et les acquisitions PNA. V0.4.md précise les 45 opérations DSP/HAL et leurs profils de recherche. ARCHITECTURE.md détaille les frontières natives/SDK. VALIDATION.md sépare tests locaux, CI et matériel non validé.')
para('Les prochaines validations dépendent des modèles, firmware, options et ressources du laboratoire. Les points prioritaires sont les dialectes des autres instruments, les calibrations applicatives, la traçabilité des acquisitions, l’ordonnanceur streaming et les plateformes mobiles.')

class NumberedCanvas(canvas.Canvas):
    def __init__(self,*a,**k):super().__init__(*a,**k);self.pages=[]
    def showPage(self):self.pages.append(dict(self.__dict__));self._startPage()
    def save(self):
        total=len(self.pages)
        for state in self.pages:
            self.__dict__.update(state);self.setStrokeColor(TEAL);self.setLineWidth(.7);self.line(52,802,543,802)
            self.setFont('Guide',7.4);self.setFillColor(GRAY);self.drawString(52,812,'RF WORKBENCH  /  GUIDE UTILISATEUR  /  0.5')
            self.drawString(52,27,'Prototype RF et métrologie  |  7 octobre 2026');self.drawRightString(543,27,f'{self._pageNumber} / {total}')
            super().showPage()
        super().save()

output=HERE/'Guide-utilisateur.pdf'
doc=SimpleDocTemplate(str(output),pagesize=A4,leftMargin=52,rightMargin=53,topMargin=55,bottomMargin=48,title='RF Workbench 0.5 - Guide utilisateur',author='RF Workbench',pageCompression=1)
doc.build(story,canvasmaker=NumberedCanvas)
print(json.dumps({'pdf':str(output),'bytes':output.stat().st_size,'chapters':24,'synthetic_Rapp_p':q},ensure_ascii=False))
