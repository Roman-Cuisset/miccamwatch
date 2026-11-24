# MicCamWatch v0.22.0 — audit de l’étape 4

Base `e773cbf266cc25471a627265d6379cf627ebcfa5` sur
`work/flutter-hybrid-0220`. Les validations sont rattachées au code qu’elles
ont réellement exécuté. Les preuves [d’installation hybride](HYBRID_DISTRIBUTION.md)
et [de récupération MSI](WINDOWS_MSI_MAINTENANCE.md) sont conservées.

## Registre d’audit

P0 couvre l’autorité des actions et la véracité des états, P1 la qualité
nécessaire à la livraison, P2 les limites de confort et P3 la dette d’entretien.
Les familles Windows, Linux et macOS couvrent respectivement x64, x64/ARM64
et Intel/Apple Silicon.

| Domaine | Gravité, cibles et constat | Correction ou décision à vérifier |
| --- | --- | --- |
| Microphone | P1, trois OS. WASAPI, PipeWire et CoreAudio ont des sources et des limites distinctes. La lecture d’un périphérique ne prouve pas qu’un client l’utilise. | Vérifier une transition réelle avec un client identifié sur chaque OS. Faire suivre une interruption du collecteur par une récupération et contrôler l’absence de STOP inventé. |
| Caméra | P0, macOS. L’inventaire vide ou toutes les lectures CoreMediaIO manquantes étaient affichés comme un collecteur dégradé, comme un relevé complet sans attribution. | Signaler le collecteur indisponible lorsque aucun état de fonctionnement n’est connu. Rejouer des états actifs, arrêtés, nuls et partiels sur les deux architectures, puis confirmer les transitions physiques au réveil. |
| Attribution | P1, trois OS. Une caméra CoreMediaIO active est visible sans client. Les chemins déclarés par un client PipeWire et certaines identités de consentement Windows n’ont pas tous la même autorité. | Conserver les identités inconnues et la provenance de chaque preuve. Vérifier la non-admission d’un client anonyme aux actions ciblées. |
| Notifications | P1, macOS et Linux. Le helper appelait la demande d’autorisation à chaque notification et bloquait la boucle pendant la livraison. Le succès de soumission ne mesure ni le style d’alerte ni son affichage. | Lire UserNotifications avant de demander l’autorisation. Confier la livraison à une file bornée qui relit les préférences et laisse la collecte continuer. Vérifier saturation, autorisation, refus, alertes désactivées, redémarrage et bannière observée. |
| Politiques | P1, trois OS. L’identité inconnue donne une décision inconnue. Les gardes de session et contrôles de périphériques dépendent de l’OS. | Conserver les garde-fous natifs, l’acquittement, la relecture et l’annulation. Tester l’absence d’effet quand l’identité ou la protection est inconnue. |
| Terminaison | P0, macOS. Vorssaint a subi un refus Mach 5 après identification. Avec sudo, le contrôle d’UID de v0.21.0 aboutissait à un autre refus. PPID 1 n’établit pas la propriété du processus. | Vérifier le compte ordinaire et l’instance avant l’acquisition du port. Revalider le port, les droits et la session au moment d’agir. Distinguer la limitation de macOS du refus interne. Le cas Vorssaint reste ouvert. |
| Historique et diagnostics | P1, Unix. Un changement de détail invisible déclenchait l’impression du statut entier. P1, Windows. Le TUI interprétait la disparition d’une ligne comme un STOP même pendant un trou de collecte. P2, trois OS. Le lecteur parcourait les fichiers à rebours octet par octet. | Émettre les motifs de santé par collecteur et les trous JSON. Reprendre une référence par ressource après un trou du TUI. Lire par blocs en conservant la pagination et ses bornes. Tester états partiels, rotation, troncature et reprise entre blocs. |
| Profils | P1, trois OS. Privé donne Strict, Développement Conservative, Équilibré et Réunion Balanced. Seul l’observateur Windows applique la différence Strict, en reclassant les accès inexpliqués. | Expliquer les effets observables et conserver l’indication de la configuration native qui prévaut. Exercer les quatre choix dans les sept langues. |
| Tray natif | P2, trois OS. AppKit utilise déjà une image template. Windows et Linux ont leurs propres chemins d’affichage natif et de mesure de largeur. | Conserver le tray permanent. Vérifier les menus, les dimensions, le focus et la lisibilité sur les deux thèmes sans remplacer les composants natifs. |
| Flutter | P1, trois OS. La navigation verticale fixe pouvait déborder après réduction de la hauteur ou agrandissement des caractères. | Rendre la navigation défilable et conserver les commandes accessibles au clavier. Mesurer l’ouverture et la disparition effective de Flutter. |
| Fenêtres | P1, trois OS. Les dimensions par défaut étaient fixes, indépendamment de l’espace utilisable. | Borner l’ouverture à la zone utile et réduire le minimum. Mémoriser le cadre AppKit et le borner après restauration. Vérifier petit écran, DPI, réouverture et retrait d’écran. La persistance Windows/Linux reste ouverte. |
| Paramètres | P1, trois OS. Flutter conserve la dernière valeur Rust confirmée. Une valeur envoyée ou un IPC déconnecté ne prouve pas l’écriture. | Rejouer le changement de profil sous acquittement retardé, reconnexion et configuration native explicite. Les nouvelles descriptions suivent les valeurs confirmées. |
| Installation et maintenance | P1, macOS. Les runners montrent `/Applications` en `root:admin 0775`. D’autres membres du groupe peuvent écrire. Les ACL de modification sont refusées. Windows MSI reste soumis aux politiques du compte. | Autoriser le répertoire personnel sûr déjà pris en charge et expliquer le refus partagé. Ne pas ajouter d’exception `root:admin` sans preuve équivalente sur tout le chemin. Rejouer les tests ACL, MSI administrateur et en vérifier la provenance. |
| Internationalisation | P2, trois OS. Un choix de langue existe pour la fenêtre. Des libellés du moteur et des détails techniques natifs demeurent en anglais. | Les cinq explications de profil sont traduites. Vérifier les textes longs et recenser les autres chaînes utilisateur non localisées. Garder les codes d’erreur recherchables. |
| Accessibilité | P1, trois OS. Les boutons natifs et Flutter fournissent le focus et certains identifiants. Le contraste, les lecteurs d’écran et les très grands facteurs de texte restent insuffisamment mesurés. | Exercer navigation, focus, réduction de mouvement, texte agrandi et lecteur d’écran sur les interfaces réelles. Une capture d’écran ne qualifie pas le lecteur d’écran. |
| Performances | P1, trois OS. Le cycle de vie Flutter est séparé. Les anciens chiffres Windows/Mint ne représentent pas une comparaison v0.21/v0.22. La vérification Authenticode Windows et le helper macOS contribuent à la charge. | Mesurer une série comparative à charge et cadence égales. L’amélioration locale d’historique a sa mesure propre ci-dessous. Conserver la fréquence de capture et les validations de confiance. |
| Erreurs | P1, trois OS. Une raison de diagnostic, une restriction système et un échec de produit appellent des réponses différentes. La CI hébergée Windows standard peut renvoyer `1625` sous `DisableMsi=1`. | Conserver la raison de refus et rechercher ce code dans le journal avec le contexte de politique. Corréler les preuves du compte standard de la VM x64 avant toute conclusion. |
| Redémarrage | P1, trois OS. La v0.22 conserve un propriétaire Rust et des données sur disque. Les preuves de reprise MSI sont propres au code de l’étape 3. | Vérifier l’autostart, la session IPC 10 et les alertes après ouverture de session. Exercer la reprise après veille sur matériel autorisé. |
| Cinq cibles | P1. Les outils natifs, les cibles Linux glibc et les paquets macOS ont des preuves distinctes. | Consigner pour chaque workflow le SHA, l’architecture et les critères réellement exécutés. Examiner les preuves déjà produites avant toute répétition. |
| Limites des API | P0/P1, macOS et Linux surtout. L’état CoreMediaIO ne donne ni client ni preuve de flux. Les API publiques macOS ne donnent pas le verrouillage courant. V4L2 peut contourner PipeWire. | Garder les capacités indépendantes. Tester les états indisponibles et les refus. Les nouveaux états ne déduisent aucun PID ni état de protection. |

P3 commun. `cargo fmt --all --check` retourne 1 sur une extraction de
`e773cbf`, avec 406 blocs d’écarts. Les zones Rust ajoutées pendant
l’étape 4 ont été alignées sur rustfmt. Les actions Flutter explicites
exécutent leurs propres compilations, Clippy et suites.

## Terminaison et autorité macOS

`libproc` fournit la naissance et le chemin du processus. La validation
vérifie maintenant l’UID réel, effectif et sauvegardé, le drapeau système
et les exécutables protégés avant `task_for_pid`. Le port Mach retenu
doit porter le même PID/version et la même session d’audit que l’appelant.
L’identité et le jeton sont relus avant `task_terminate`. Un PID recyclé
ne change pas la cible du port retenu. Les confirmations existantes et
la fenêtre de confirmation IPC de trois secondes restent en vigueur.

Apple documente `NSRunningApplication.terminate()` pour demander une
fermeture normale à une application enregistrée. Les objets sont à comparer
par identité d’instance. Cette interface pourrait servir à une action
explicite de fermeture, avec confirmation de sortie et après preuve dans
une session native. Elle n’est pas adoptée comme repli automatique pour
un PID, un service ou un refus de port Mach. Les variantes par jeton
apparaissent dans les sources XNU, sans contrat API public exploitable
trouvé pour cette action. [Identité AppKit](https://developer.apple.com/documentation/appkit/nsrunningapplication/processidentifier)
· [Fermeture AppKit](https://developer.apple.com/documentation/appkit/nsrunningapplication/terminate%28%29)
· [Sources XNU](https://github.com/apple-oss-distributions/xnu/blob/main/libsyscall/wrappers/libproc/libproc.c).

À qualifier avec un processus jetable, un autre compte, un processus
protégé, une identité expirée, une annulation et un refus d’autorité.
Les tests de refus et de confirmation IPC ne prouvent pas à eux seuls
qu’un vrai processus utilisateur durci est terminable.

## Mesure ciblée de l’historique

Comparaison du scanner de `e773cbf` et du scanner par blocs, rustc 1.98.1
`-O`, sur l’hôte Windows x64 avec des fichiers synthétiques indépendants.
Chaque scénario alterne AB/BA pendant six tours de 32 scans par variante.
Les deux variantes conservent la limite de page de 512 Kio et tous les
résultats de lecture, y compris les lignes trop longues. Les médianes
ci-dessous mesurent le scanner avec fichiers chauds, ouverture/lecture
incluses. Le décodage JSON, l’IPC, le moteur Flutter et la mémoire ne font
pas partie de ce test.

| Charge | Données | Avant | Après |
| --- | ---: | ---: | ---: |
| Lignes de 800 octets | 4 921 344 octets et 6 144 lignes | 23,57 ms | 11,78 ms |
| Lignes de 63 Kio | 4 773 962 octets et 74 lignes | 30,04 ms | 15,66 ms |
| Lignes au-delà du maximum | 4 804 677 octets et 69 omissions attendues | 28,42 ms | 13,57 ms |

La comparaison complète doit suivre le [protocole de release](RELEASE_0.22.0_PLAN.md#7-protocole-reproductible-cpuram-et-démarrage).
Elle doit mesurer repos, micro actif, caméra active, collecteur dégradé,
Flutter au repos et actif, fermeture, changement de thème/langue et reprise.
Relever le CPU moyen et les pics, la RSS, la mémoire privée, les processus
et threads, les cadences et les latences. Sur Windows la campagne de
comparaison sous profil jetable peut réutiliser
`desktop/qualification/compare_cpu.py` lorsque les deux préfixes sont
comparables. Une session hébergée sans capteur qualifie la dégradation
logicielle et laisse ouverte la campagne matérielle.

## Qualification sur `b3350bc`

| Cible | Résultat |
| --- | --- |
| Linux x64 | Bundle Ubuntu 22.04, installation hybride et cycle dans un bureau X11 isolé. [Job](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45402576893/job/138474736513) |
| Linux ARM64 | Compilation et qualification du bundle sous Ubuntu 22.04 natif. [Job](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45402576893/job/138474736840) |
| macOS Intel | Bundle et cycle AppKit/Flutter en session hébergée. [Job](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45402576893/job/138474736479) |
| macOS Apple Silicon | Bundle et cycle AppKit/Flutter en session hébergée. [Job](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45402576893/job/138474736637) |
| Windows x64 administrateur | Rust et Flutter, MSI et ZIP audités, upgrade, rollback, enregistrement, fermeture Flutter avec tray maintenu et désinstallation. [Job et reçu](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45402577173/job/138474740096) |
| Windows x64 compte standard hébergé | Le compte a passé la vérification du jeton. L’installateur public v0.21.0 a reçu 1625 avec `DisableMsi=1` avant tout essai de la candidate. [Job et reçu](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45402577173/job/138474740096) |

Le SHA-256 du MSI `b3350bc` a été revérifié sur l’artefact CI
`5e162a601423ab3ac5f7a8da839351bc13f1233e24de7ac25db7548dc11b0514`.
La preuve de récupération standard de l’étape 3 reste attachée à son
propre paquet. Le comparatif Windows ajouté après cette révision
échantillonne les processus du paquet installé sur le même runner, avec
la v0.21.0 et la candidate, avant/après fermeture. Il fournit un premier
relevé court à interpréter selon la couverture aux deux bornes.

### Linux x64, fenêtre ouverte sous Xvfb

Dans le reçu `runtime/lifecycle/result.json` de `b3350bc`, les deux
échantillons de 15 s avec tray seul donnent 0,20 à 0,27 % d’un cœur et
environ 9,45 Mio de RSS. Après fermeture, il reste un processus à
9,83–9,90 Mio de RSS et 0,13 % d’un cœur. Les deux premières fenêtres
ouvertes utilisent 159,88–162,09 % d’un cœur pour l’arbre, avec
299,06–299,38 Mio de RSS et 251,18–251,33 Mio de PSS. Dans le premier
échantillon clair, Flutter a consommé 24,32 s de CPU et Rust 0,03 s.
Le moteur graphique logiciel est observé dans les deux essais.
L’ouverture jusqu’à fenêtre native prête a pris 145–327 ms sur six
cycles, et l’arrêt du processus Flutter 107–318 ms.

Cette charge rend la performance de la fenêtre Linux ouverte
insuffisamment qualifiée. Les mesures portent sur Xvfb et un runtime
de qualification sans capture physique. Elles localisent la charge
au processus Flutter. Le coût par thread et la cause exacte restent
à départager entre rendu logiciel, moteur et rafraîchissements de
l’interface. Le même binaire devra être mesuré en Xvfb avec chaque
renderer pris en charge et sur une session accélérée, avec CPU par
thread. Aucun changement de renderer ni de cadence native n’a été
déduit de ces chiffres. [Reçu Linux x64](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45402576893/job/138474736513)
· [Options du moteur de rendu Flutter](https://docs.flutter.dev/perf/impeller).
