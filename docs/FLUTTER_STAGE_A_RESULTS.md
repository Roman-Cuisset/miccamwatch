# MicCamWatch 0.21.1 développement — livraison Windows A

État vérifié le **28 octobre 2026**, branche locale `work/flutter-hybrid-0211`. Sources finales compilées : **34e4e94c170565ef18482280241f9efb4a59a283**. Étape A fonctionnelle, à faire valider par le propriétaire ; **ni migration complète ni version publiable**. Aucun push, fusion main, tag, package installé ou déploiement.

## Périmètre livré

- Vrai observateur/tray Rust, vrais collecteurs et états natifs ; aucune donnée de démonstration dans les captures de cette tranche.
- Une commande explicite `set_microphone_protection(requested: bool)`, reçue par le superviseur puis exécutée par le **worker/API Rust existants**. Pas de contrôle matériel en Dart, nouveau mécanisme de blocage ou nouvelle élévation.
- Protocole2 sur pipes hérités privés : défi CSPRNG128bits par enfant, ID monotone, génération/intention attendues, confirmation distincte mute/release, une opération en cours. Paramètres/versions stricts ; pas de shell, chemin, appareil ou privilège fourni par Flutter.
- État accepté/en cours distinct du résultat et de l'état effectif. Succès seulement après relecture SDK/intention, avec garde active requise pour la demande de sourdine. Échec partiel ou timeout : incertain/échec, jamais une protection inventée, jamais un retry automatique.
- Fermeture réelle de Flutter, sans libérer une garde manuelle par effet de bord. Réouverture et activation de la même fenêtre ; travail natif indépendant de l'enfant graphique.
- Boutons SDK explicites, confirmation bornée à560pxlogiques et défilable, thèmes clair/sombre, logo PNG officiel latéral et ICO officiel en titre. Aucun plugin Flutter runtime ajouté.
- « Sources de surveillance : opérationnelles » remplace l'ancien libellé ambigu « couverture complète » dans les sept langues. Des collecteurs sains ne prouvent pas toutes les captures possibles. Le backend conserve ses valeurs de couverture.

**Sémantique Windows :** SDK mute n'est pas refus d'autorisation ni coupure matérielle universelle. Capture exclusive/ASIO/chemins non pris en charge peuvent le contourner. Le release rétablit toutes les entrées actives prises en charge ; **il ne restaure pas leurs anciennes valeurs individuelles**. Ces limites figurent dans l'interface et les deux confirmations.

Le tray opérationnel conserve ses fonctions et son accès aux réglages/contrôles natifs. Son raccord Flutter est compilé, mais n'a pas été lancé sur l'installation quotidienne afin d'éviter une reprise concurrente de protection. Les essais utilisent un préfixe et des données de développement isolés, pas un attachement au processus MSI.

## Contrats, inventaire et fichiers

- [Matrice exhaustive CLI/tray/Win32/GTK/AppKit](FLUTTER_MIGRATION_MATRIX.md) : disponibilité, force réelle, autorisations, restauration, préférences, maintenance et écarts connus.
- [Contrat IPC et qualification de sécurité](FLUTTER_COMMANDS_STAGE_A.md).
- Rust : `src/frontends/desktop.rs`, `src/frontends/desktop/commands.rs`, `src/frontends/desktop/windows.rs`, `src/frontends/tray.rs`, `src/frontends/tray/windows_audio.rs`, `src/frontends/tray/windows_observer.rs`, `src/bin/mcw-tray.rs`.
- Flutter : `desktop/flutter/lib/{commands,main,snapshot,labels}.dart`, asset officiel et déclaration `pubspec.yaml`, version du protocole dans le runner Windows.
- Tests : `desktop/flutter/test/{commands,snapshot}_test.dart`, tests Rust de contrat/DTO/readback.
- Outillage : `desktop/qualification/windows.py` et `cpu_threads.py`. Les sondes jetables du collecteur ont été retirées ; `src/platform/windows.rs` n'a pas été modifié par le diagnostic final.

Les rapports [de première intégration](FLUTTER_INTEGRATION.md) et [de première tranche](FLUTTER_0.21.1_FIRST_SLICE.md) restent historiques, épinglés au protocole1 et à leurs anciens commits. Ils ne décrivent pas le schéma courant et leurs chiffres ne sont pas réattribués à cette étape.

## Vérifications exécutées

| Vérification | Résultat et portée |
|---|---|
| Rust `cargo check --features windows-tray,flutter-desktop --all-targets` | Réussi lors du raccord ; compilation complète du chemin Windows feature-enabled |
| Rust `cargo test --features windows-tray,flutter-desktop --lib frontends::desktop` | **26 réussis**,191 filtrés ; DTO, admission, replay/session/génération, bornes et confirmations/readbacks. Ce n'est pas la suite Rust complète |
| Rust Clippy, mêmes features, `--all-targets -- -D warnings` | Réussi sur les sources finales |
| Rust release `mcw` + `mcw-tray` | Réussi sur34e4e94 |
| Native sans Flutter : `cargo check --features windows-tray --all-targets` | Réussi ; aucune modification ultérieure de ce chemin hors feature |
| Flutter `analyze` | Aucune anomalie sur les sources finales |
| Flutter `test` | **27 réussis** :8cas snapshot/fraîcheur,19cas client/panel, confirmations, échecs, timeout, navigation, session/génération et gates |
| Flutter Windows release/AOT | Réussi ; SDK3.47.8/Dart3.13.7 réutilisés, pas d'installation globale |
| Refus de périmètre | Exécution contrôlée avec les répertoires utilisateur réels hors qualification : refus avant démarrage du tray/worker, dialogue explicite, sortie2 après fermeture du seul dialogue possédé |
| SDK réel depuis Flutter | Sourdine puis release confirmés, une entrée observée ; voir scénario ci-dessous |
| Fenêtre/processus | Neuf ouvertures au total, dont huit réouvertures et une rafale d'activations ; exactement un enfant graphique ouvert, aucun après chaque fermeture |
| Visuel final34e4e94 | Vrais HWND capturés en clair et sombre, français,125%/DPI120 ; aucun ordre matériel pendant cette vérification finale |

Les échecs initiaux Flutter provenaient de timers de fixtures non libérés avant les invariants du binding ; corrigés sans annuler une opération native lors du démontage d'un panel. Les premières captures blanches/échecs PrintWindow ou frame WGC indisponible sont conservés comme **échecs**, pas comme validations. La passe graphique réussie utilise Windows.Graphics.Capture sur le HWND/PID possédé, sans capture du bureau. Une requête d'affichage temporaire du thread est restaurée dans `finally` ; aucun réglage d'alimentation persistant ni déplacement de souris globale.

Les erreurs SDK/readbacks partiels et rejets d'authentification sont couverts par les tests de contrat. Pas de refus UAC microphone fabriqué : cette API n'en demande pas. Pas de déconnexion physique de périphérique ni de conflit créé avec une protection utilisateur pour forcer une erreur. La fermeture **pendant une opération native encore en cours** n'a pas été provoquée en essai matériel ; indépendance du worker, timeout et absence de retry sont distingués de la fermeture **après activation**, réellement exercée.

## Cycle matériel autorisé et réversible

Runtime du cycle : backend `a8f23e7`, UI reconstruite proprement sur `05d50ee`. Préflight : toutes les entrées observées rétablies, aucune garde/demande active, collecteurs sains et aucun flux Active observé. APPDATA/LOCALAPPDATA isolés sous le dossier de qualification. Une entrée annonçait la capacité de sourdine matérielle ; cela ne prouve pas un blocage universel.

1. Ouvrir la confirmation SDK ne produit aucun ordre natif.
2. Confirmation Flutter → ticket2 : `requested=true`, résultat `succeeded`, SDK `Muted`, garde active,1entrée. Résultat natif observé après **459ms**.
3. Fermer Flutter : enfant sorti, compteur d'observation **430→435** ; seuls tray Rust et garde native subsistent. La garde n'est pas un processus Flutter orphelin.
4. Trois demandes d'ouverture : un seul enfant ; demande de sourdine et garde conservées.
5. Confirmation distincte de release → ticket3 : `requested=false`, `succeeded`, SDK `Unmuted`. Résultat observé après **411ms**.
6. Relecture finale : SDK unmuted, aucune demande, garde arrêtée ; tous les PID de test sortis normalement. Caméra non modifiée.

Ces deux délais sont des observations uniques **confirmation UI → résultat natif disponible**, pas des médianes d'API. Le retour de release peut observer brièvement une garde encore vivante alors que demande=false et SDK=unmuted ; son arrêt a été confirmé ensuite.

Après ce cycle, seules des précisions de texte/sémantique d'affichage ont été ajoutées sur34e4e94, sans changer l'admission ni le contrôle natif. Compilation,26tests Rust,27tests Flutter et rendu clair/sombre ont été revérifiés. **Le cycle physique n'a pas été répété le28octobre** pour ces textes.

## Ressources et latences du build final

Mesure Windows11 x64 locale,12processeurs logiques, builds release sans débogueur ni SDK Flutter attaché. L'observateur de mesure est **en lecture seule**, avec le même protocole2 et le vrai moteur, pas le tray MSI installé. Échantillons10s fermé avant,15s ouvert,15s fermé après neuf cycles. L'essai fermé avec worker contrôlé est une passe séparée, sans fenêtre ni commande.

| État | Processus | Somme RSS, Mio | Commit privé, Mio | Working set privé USS, Mio | CPU, % d'un cœur logique |
|---|---:|---:|---:|---:|---:|
| Rust lecture seule avant ouverture | 1 | 21,51 | 3,92 | 3,01 | 10,63 |
| Rust + Flutter ouvert | 2 | 133,55 | 117,26 | 74,37 | 9,75 |
| Rust après neuf fermetures | 1 | 22,54 | 4,21 | 3,29 | 10,01 |
| Rust avec worker contrôlé, fermé, passe distincte | 1 | 19,28 | 4,00 | 3,06 | 6,96 |

RSS inclut les pages partageables et sa somme peut les compter plusieurs fois ; commit privé n'est pas mémoire résidente. Ces chiffres n'incluent pas la mémoire GPU/DWM. La différence de RSS entre passes ne prouve ni optimisation du worker ni absence de toute fuite native ; l'invariant réellement vérifié est l'absence d'accumulation de processus Flutter.

| Mesure | Résultat |
|---|---:|
| Premier `ready` du nouveau parent, un lancement | 944ms |
| Huit réouvertures, médiane | 239ms |
| Huit réouvertures, min–max | 220–254ms |
| Neuf fermetures jusqu'à sortie du processus, médiane | 239ms |
| Payload Flutter distribué | 25,34Mio |
| Payload complet CLI + tray + Flutter | 37,47Mio |

**`ready` signifie instantané valide et callback de layout Flutter**, pas preuve de premier pixel réellement affiché par Windows. Le temps exact du premier raster exploitable n'a pas été mesuré dans cette série ; ne pas le présenter comme944ms/239ms. Premier lancement = premier enfant de ce parent, sans purge artificielle des caches Windows ; ce n'est pas un démarrage froid du système. Les tailles excluent le manifeste local de qualification et les outils de capture/mesure.

## CPU : cause mesurée, pas un correctif annoncé

La mesure précédente sur60s, Flutter absent, était **47,79% d'un cœur**, dont47,71% sur un seul thread non principal. Les sondes retirées ont attribué le coût au chemin caméra : médiane registre/modules960,94msCPU par appel ;20vérifications de signatures totalisaient8531,25msCPU **imbriquées** dans9593,75msCPU de registre/modules. L'activité sensorielle mesurait31,25msCPU pour534,70msmur : attendre500ms n'est pas brûler500msCPU. Ne pas additionner les coûts imbriqués.

Le28octobre, un seul pipeline caméra Ready était observé, contre deux lors du diagnostic initial. Les chiffres CPU plus faibles de la nouvelle passe **ne prouvent pas un correctif** : le code du collecteur, les signatures et les cadences n'ont pas été modifiés. Le coût reste dépendant du contexte à qualifier. Aucun cache de confiance par chemin/mtime, diminution de fréquence ou exclusion de collecte n'a été ajouté.

Prochaine optimisation à concevoir séparément : réduire le coût de vérification répétée **sans perdre l'identité des fichiers/processus, la politique de confiance et ses invalidations**. Ne pas résoudre cela en ignorant les pipelines Ready ou en attribuant le coût à Flutter.

## Captures, reçus et exécutable à examiner

Racine durable locale : `%LOCALAPPDATA%\MicCamWatchQualification\flutter-commands-stage-a`.

- `runtime-qualified\` : payload complet34e4e94 et `qualification-manifest.json`,15fichiers vérifiés SHA-256. Le manifeste est une preuve locale d'intégrité, **pas une signature d'éditeur ou une attestation**.
- `qualified-controls-light.png`, `qualified-controls-dark.png` et `qualified-controls-captures-20261028.json` : rendu final et arrêt de tous les PID, zéro ordre matériel.
- `final-interactive-controlled-cycle.json`, `final-confirmed-muted-light.png`, `final-confirmed-released-dark.png` : vrai cycle matériel sur les sources indiquées, données non simulées.
- `resources-20261028-082339.json` : neuf cycles et ressources, sauvegarde après chaque phase.
- `controlled-closed-resources-20261028.json`, `context-20261028.json`, `qualified-scope-denial.json` : worker fermé, contexte et vrai refus.
- `collector-parts-profile.{stdout,stderr}.jsonl`, `cpu-baseline-closed-60s.json` : attribution CPU et mesure initiale.
- `flutter-{analyze,tests}-qualified.log`, `flutter-qualified-windows-release.log`, `final-safety-20261028.json` : vérifications finales.
- `capture-tool\` : source/lock/exécutable de la sonde WGC mono-image, compilée hors dépôt avec le crate Windows0.62.2 disponible. Ce n'est pas un composant de MicCamWatch distribué.

Pour examiner le rendu **sans commandes matérielles**, lancer uniquement le préfixe de développement, jamais le raccourci MSI ni `mcw` résolu dans le PATH :

```powershell
$p = Join-Path $env:LOCALAPPDATA 'MicCamWatchQualification\flutter-commands-stage-a\runtime-qualified'
Start-Process -FilePath "$p\mcw-tray.exe" -WorkingDirectory $p -ArgumentList '--read-only-development','--open','--desktop-theme','light'
```

Fermer la fenêtre termine Flutter ; quitter ensuite **le tray de développement**. Pour le mode contrôlé, suivre le [périmètre de qualification](FLUTTER_COMMANDS_STAGE_A.md#qualification-boundary), avec les deux répertoires de données isolés précréés et le préflight SDK ; ne pas le lancer avec les données utilisateur réelles. Ne pas utiliser un release global pour « restaurer » un microphone initialement déjà coupé.
Les deux modes de développement partagent un singleton : quitter le tray de développement existant avant de changer de mode. Une nouvelle invocation n'est pas un « downgrade » des capacités d'un propriétaire contrôlé déjà actif. Le bandeau de la fenêtre indique son mode réel.

Mesure de processus reproductible depuis le worktree, sans modifier les périphériques :

```powershell
python desktop/qualification/windows.py --prefix "$p" --out "$env:LOCALAPPDATA\MicCamWatchQualification\stage-a-repeat" --trials 1 --cycles 8 --idle-seconds 15
```

`--captures` est optionnel et absent de cette commande : cette passe ne valide pas le rendu. PrintWindow peut échouer avec une surface GPU/occultée ; ne pas masquer cet échec ni capturer le bureau en remplacement.

## Préservation et limites

- Exécutables MSI0.21.0 : SHA-256 inchangés. Les6fichiers stables hors historique comparés, dont préférences et journaux d'ownership, sont inchangés. L'historique s'est accru, mais son préfixe initial est identique : aucune suppression ni réécriture constatée.
- Inventaire final : seul le tray MSI installé reste actif ; aucun processus Flutter, garde SDK ou observateur expérimental laissé en marche. Le PID stable a changé après l'interruption nocturne ; aucune hypothèse fondée sur l'ancien PID n'a servi au nettoyage.
- Aucun paramètre caméra, autorisation, autostart, PATH, installation, autre worktree ou branche publique modifié par cette tranche.
- Linux x64/ARM64, macOS Intel/Apple Silicon : interfaces natives conservées ; transport/commandes/packaging Flutter **non qualifiés**, aucune migration anticipée ni validation matérielle ARM64 revendiquée.
- Sept langues présentes pour les nouveaux contrôles/limites ; validation visuelle réelle limitée au français125%. Clavier/lecteur d'écran complets,100%/150% DPI et toutes les langues restent à qualifier avant finalisation graphique.
- Pas de commande Flutter caméra, préférences, historique, autostart, mise à jour ou désinstallation dans cette livraison. Les fonctionnalités natives restent les références ; pas de faux pouvoirs ajoutés pour remplir la matrice.

## Point d'arrêt et reprise

**Arrêter ici pour validation du propriétaire.** Ne pas migrer les autres commandes ou plateformes, installer, pousser, fusionner, taguer ou publier sans nouvel accord. Faire examiner le rendu, le vrai contrôle SDK et les ressources ; traiter ensuite le coût CPU de signatures et choisir le lot de parité suivant. Préserver les limites de mute/release, le mode en lecture seule, l'indépendance du worker, les interfaces natives et tous les reçus historiques.
