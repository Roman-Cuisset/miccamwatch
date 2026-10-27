# MicCamWatch 0.21.1 — première tranche hybride réelle, Windows

État du 27 octobre 2026. **Tranche de développement fonctionnelle, pas une version publiée ni une migration complète.** Branche locale `work/flutter-hybrid-0211`, base `73369abda09bdadce8c779a9559b3e2334a32ecb`. Sources runtime qualifiées : **743f5115dfa923404b3fbdd0427e86c1f28b0f27** ; le commit documentaire suivant conserve ce bilan. Aucun push, tag, installateur, mise à jour du MSI stable ou déploiement.

## Ce qui fonctionne

- Vrai tray Rust de développement et vrais collecteurs `PlatformMonitor`, pas le moteur inerte de l'étude comparative.
- Fenêtre Flutter Windows release/AOT ouverte à la demande, alimentée par des instantanés réels du moteur Rust : micro/caméra, activité, disponibilité, confiance, couverture, applications observées, santé des collecteurs et diagnostics.
- État réel passif, demande de protection et service/garde séparés. Aucune commande matérielle, préférence, autorisation ou restauration n'est exposée par le protocole Flutter.
- Fermeture qui termine effectivement Flutter ; collecte Rust continuée et compteur avançant. Réactivation de la fenêtre visible sans deuxième enfant ; réouverture après fermeture et après arrêt forcé du seul enfant graphique.
- Job Object contenant l'enfant avant exécution ; arrêt du parent testé avec fenêtre ouverte, sans PID possédé restant.
- Vue d'ensemble, diagnostics et À propos, thèmes clair/sombre, logo Windows officiel. Navigation vers Diagnostic effectivement exercée dans le HWND Flutter possédé, sans déplacer la souris globale.

**Périmètre de la preuve :** le binaire de développement exécute un observateur supplémentaire en lecture seule, avec la politique de collecte par défaut. Il ne s'attache pas au processus MSI déjà installé. Les lectures de protection conservent le périmètre/les limitations des API Rust appelées ; elles ne revendiquent pas le contrôle d'une autre namespace. Le raccord dans le tray opérationnel est compilé et les fonctions natives restent accessibles, mais il n'a pas été lancé sur ce PC pour éviter toute reprise de protection concurrente. Ce n'est pas une qualification de package ou de contrôle matériel.

Le moteur garde volontairement sa version crate `0.21.0` dans cette tranche ; la fenêtre indique `0.21.1 development`. Cela ne publie pas 0.21.1 et ne modifie pas les contrats des gardes installées.

## Méthode et environnement

Windows 11 Pro Insider 10.0.26220 x64, Intel i5-1235U/12 processeurs logiques, 34 089 381 888 octets de RAM, Iris Xe/pilote 32.0.101.7092. Affichage réel **125 % / DPI120**, 1920 × 1080. Flutter **3.47.8**, Dart **3.13.7**, SDK déjà vérifié pendant l'étude, réutilisé sans installation globale. Aucun plugin runtime supplémentaire : SDK Flutter uniquement.

Trois essais indépendants pour le rendu Windows par défaut, trois pour le runtime final. Chaque essai comprend une première ouverture et trois réouvertures, avec mesures de quatre secondes par phase. Le runtime final ajoute dans chaque essai une activation visible répétée dix fois, un arrêt forcé de l'enfant puis une réouverture, et l'arrêt du parent avec enfant ouvert. Les deux suites principales représentent **24 cycles ordinaires**, plus six ouvertures de résilience finales. Le sombre est vérifié séparément : un essai et trois cycles ordinaires. Ce sont des séries courtes, pas un test d'endurance.

Builds optimisés, pas `flutter run`, debugger, moteur préchauffé, trim mémoire, changement de priorité, exclusion antivirus ou modification du DPI/thème Windows. Aucune compilation concurrente pendant les deux séries principales. PC utilisé normalement, pas un banc exclusif ; effets de cache, ordonnancement et GPU non isolés. L'ordre par défaut puis Skia peut favoriser les caches du second ; aucune promesse de latence sur un autre PC.

## Délai et rendu

L'initialisation du moteur graphique dominait, bien avant l'affichage des données ou l'IPC. Le runner Windows utilise maintenant l'API officielle `DartProject::set_impeller_switch(Disabled)` pour le chemin Skia. Linux/macOS conservent leurs propres paramètres ; rien n'y est qualifié par ces chiffres.

Mesure principale : réception par le parent de `ready` après rendu d'un instantané réel de séquence positive, relative à la demande d'ouverture traitée par le superviseur. Ce n'est pas une mesure physique photon-à-photon. Les mesures depuis l'envoi du message jusqu'au HWND visible sont conservées séparément dans les journaux.

| Mesure | Rendu Windows par défaut | Runtime final Skia |
|---|---:|---:|
| Première ouverture exploitable, médiane de 3 | 3 765 ms | 806 ms |
| Première ouverture, plage | 2 171–3 852 ms | 513–1 870 ms |
| Réouverture exploitable, médiane de 9 | 4 970 ms | 424 ms |
| Réouverture, plage | 3 568–14 382 ms | 312–4 625 ms |
| Création du moteur, médiane des 12 ouvertures | 4 265,5 ms | 273,5 ms |
| Fermeture jusqu'à disparition du processus, médiane des 12 | 390,6 ms | 353,5 ms |
| Fenêtre visible depuis le message, première ouverture médiane | 3 849,6 ms | 934,2 ms |
| Fenêtre visible depuis le message, réouverture médiane | 5 207,4 ms | 549,4 ms |

**Limite persistante :** une réouverture Skia prend encore 4 625 ms, dont 4 281 ms de création du moteur. L'optimisation réduit nettement les médianes mais ne borne pas le pire cas. La cause de cette valeur atypique n'est pas établie ; ne pas annoncer « ouverture toujours instantanée ». Aucun moteur Flutter n'est conservé après fermeture.

## Mémoire, CPU et processus

Valeurs mémoire en **Mio**, médianes des médianes des trois essais. Mémoire privée engagée ≠ mémoire résidente. RSS inclut les pages partageables ; sa somme peut les compter plusieurs fois. USS est le résident privé. Mémoire GPU/DWM non comptée ; les caches système peuvent survivre au processus. CPU100 % = un cœur logique, pas toute la machine.

| Phase | Privé engagé final | RSS final | USS final | Processus final | CPU final, médiane d'un cœur |
|---|---:|---:|---:|---:|---:|
| Rust collectant, avant première fenêtre | 4,48 | 21,67 | 3,19 | 1 | 37,82 % |
| Rust + Flutter ouverts | 112,60 | 133,25 | 70,74 | 2 | 43,87 % |
| Rust collectant, Flutter complètement fermé | 5,04 | 23,05 | 3,46 | 1 | 40,23 % |

La référence ouverte utilisait 183,35 Mio privés engagés et 197,41 Mio RSS. Le tray final repasse de onze threads pendant l'UI à neuf après fermeture dans chacun des essais ; aucun enfant ni thread de pipe accumulé dans ces séries. Cela ne prouve pas l'absence de fuite après des heures.

**CPU à surveiller :** la collecte réelle en boucle est déjà active fenêtre fermée ; ce n'est pas un faux tray au repos. La charge fermée représente environ 3,35 % de la capacité théorique des douze processeurs logiques. La variance et les fenêtres de quatre secondes ne justifient aucune revendication d'amélioration CPU grâce à Skia. Ne pas confondre ces résultats avec les chiffres du prototype à moteur inerte ni avec une mesure équivalente du MSI opérationnel.

## Taille locale non compressée

| Élément | Octets |
|---|---:|
| Bundle Flutter complet, DLL et assets compris | 26 257 244 |
| Tray Rust du préfixe final | 3 269 632 |
| CLI Rust du préfixe final | 9 339 904 |
| Total runtime CLI + tray + Flutter | 38 866 780 |

Ce n'est pas une taille MSI. Le SDK Flutter, les dépendances de qualification Python et les caches de build ne sont pas distribués avec l'application. Le préfixe et son manifeste sont conservés hors dépôt ; le manifeste local n'est ni une signature éditeur ni une attestation.

## Vérifications exécutées

- `cargo clippy --features windows-tray,flutter-desktop --all-targets -- -D warnings` : réussi sur les sources finales.
- `cargo test --features windows-tray,flutter-desktop --lib frontends::desktop` : **14 réussis**, 191 autres filtrés. Inconnus/lacunes, activité retenue, périmètres de collecteurs portables, whitelist/UTF8/bornes, refus des mutations/versions/extra-champs, framing, coalescence, thème, reparse/hardlinks et réservations du bundle.
- Builds Rust release `mcw-tray` et `mcw` avec la feature : réussis.
- Clippy `-D warnings` et build release du tray **sans** `flutter-desktop` : réussis. Le nouveau binaire natif refuse effectivement `--read-only-development` avant de lancer un tray opérationnel ; dialogue inspecté et sortie2 après fermeture par WM_CLOSE.
- Flutter analyze : aucun problème. Flutter test : **7 réussis**, protocole/fraîcheur/ordre/bornes/EOF/UTF8 et états actifs/incomplets/collecteurs portables. Build Windows release/AOT final réussi.
- Séries réelles finales : tous les scénarios demandés ci-dessus passés, progression de collecte après fermeture et après crash UI, aucun PID possédé restant après arrêt. Captures claires, sombres et diagnostics examinées au DPI125 %.
- Avertissement **MSBuild8029** conservé : worktree sous Temp pouvant affecter la génération incrémentielle. Aucun avertissement masqué.

## Essais invalides conservés

1. Premier harnais : recherche par titre d'un HWND caché effectivement vide, alors que sa classe existait. Correction classe+PID et nettoyage de l'essai, sans changer l'application pour contourner le contrôle.
2. Série Skia préliminaire : deux essais complets réussis, troisième rejeté car une fenêtre s'ouvre pendant une phase supposée fermée. Cause non établie. Marqueurs de source ajoutés ; les trois essais finaux passent et ne montrent que les activations attendues `window_activation`. L'essai rejeté n'est pas compté dans les tableaux.
3. Refus du mode natif : fermeture automatisée par WM_COMMAND inefficace ; processus de cet essai uniquement arrêté. Le contrôle suivant ferme le vrai dialogue par WM_CLOSE et observe la sortie2. Pas de correction du produit justifiée par cette différence d'automatisation.
4. Répertoire initialement nommé `final-dark` lancé par erreur sans `--theme dark` : il contient une capture **claire**, exclue de la preuve sombre. La preuve sombre est exclusivement `final-dark-verified/live-dark.png`.

## Préservation et limites

SHA-256 des deux exécutables MSI installés inchangés ; tray MSI d'origine PID9036 toujours présent lors du contrôle. Préférences, intention micro et journal utilisateur des caméras bloquées : empreintes inchangées. L'historique a grandi, mais le hash du préfixe de taille initiale confirme que **tous les octets antérieurs sont conservés** ; croissance compatible avec le tray stable déjà actif, sans attribuer son auteur par un hash seul. Le mode de développement n'instancie aucun writer d'historique.

Journal caméra sous ProgramData non lisible avec ce jeton : aucune élévation ou tentative de contournement. Les comparaisons portent sur les fichiers effectivement accessibles, pas sur une preuve exhaustive de tous les états système. Aucun flux audio/vidéo ouvert, aucune commande de protection exécutée, aucune installation changée.

- Réglages, notifications, son, profil, autostart, historique et commandes de protection restent dans les interfaces natives/CLI ; pas de faux réglages Flutter. Matrice de correspondance dans [le contrat d'intégration](FLUTTER_INTEGRATION.md#migration-matrix--first-slice-not-full-replacement).
- Libellés de base en sept langues, rendu français réellement examiné ; diagnostic natif anglais conservé. Pas de certification de parité linguistique complète, de lecteur d'écran ou de navigation clavier exhaustive.
- Windows10, DPI100/150 %, autres GPU, Linuxx64/ARM64 et macOS Intel/Apple Silicon : **non qualifiés pour cette intégration Flutter**. Les interfaces natives existantes sont conservées. Les tests portables de DTO ne remplacent pas une compilation native. Les résultats CI historiques de0.21.0 ne prouvent pas Flutter ; ARM physique reste non validé. Risques Flutter macOS Intel/Linux ARM de l'étude toujours ouverts.
- Aucune capture volontaire n'a été provoquée sur l'hôte : les captures montrent les états réels « aucune activité observée » et « disponibilité observée ». Les cas Active/incomplet/inconnu sont couverts au contrat et par les tests, pas par l'ouverture d'un flux matériel pour forcer ces états.
- Ouverture immédiate strictement concurrente à WM_CLOSE, surcharge durable de transport et endurance restent des scénarios supplémentaires, pas des succès revendiqués.

## Preuves et reprise

Racine privée durable : `%LOCALAPPDATA%\MicCamWatchQualification\flutter-hybrid-0211-20261027`.

- `final-runtime/` : exécutables et bundle complet, manifeste/SHA du runtime ; exécution manuelle et reconstruction décrites dans [FLUTTER_INTEGRATION.md](FLUTTER_INTEGRATION.md#reproduce-the-windows-slice-without-touching-the-stable-installation).
- `default-repeated/qualification.json`, `final-light/qualification.json`, `final-dark-verified/qualification.json` : mesures et événements sans payload d'instantané.
- `final-light/live-light.png`, `final-dark-verified/live-dark.png`, `live-diagnostics-dark.png` : captures authentiques de HWND possédés. Elles restent hors Git car elles montrent les noms/PID d'applications réellement observées.
- `skia-repeated/qualification.json` et preuves des essais invalides conservées ; logs de compilation/analyse/tests Flutter, `summary.json`, tailles et empreintes de préservation.

**Point d'arrêt demandé atteint.** Faire examiner cette tranche et ses captures par le propriétaire avant toute migration des réglages, des commandes sensibles ou des autres plateformes. Prochaine décision : validation du rendu et des ressources, puis choix du lot suivant. Ne pas pousser, installer, fusionner, taguer ou publier sans nouvel accord.
