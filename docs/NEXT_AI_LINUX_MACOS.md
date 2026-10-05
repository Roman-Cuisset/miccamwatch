# Phase 3 : backend macOS 15+ et validations restantes

Les backends Linux/macOS de la PR #1 sont intégrés à `main`. [La répétition release 36682926446](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/36682926446) a réussi pour Windows, Linux x64 Ubuntu 22.04, macOS 15 Apple Silicon et Intel : archives extraites exécutées, helper Swift embarqué, CLI, arrêt SIGINT et MSI Windows. Le runner Apple Silicon était sous macOS 15.7.9. Les paquets Linux/macOS de v0.14.0 restent expérimentaux : cela ne prouve pas les transitions de micro/caméra physiques. Ne pas annoncer ces preuves matérielles.

## État après implémentation de la phase 3

- `src/platform/linux.rs` respecte `CaptureCollector` : graphe PipeWire réel, PID application rapproché du `pipewire.sec.pid` authentifié du Client puis validé dans `/proc`, V4L2 uniquement `Ready` pour FD ouvert. `src/watcher/unix.rs` distingue couverture dégradée et interruption d'observation ; les commandes CLI et SIGINT ont été exécutés sous Linux et sur runner macOS. Les contrôles Windows restent rejetés sur Unix.
- Sur `serveur@serveur-asus`, PipeWire 1.0.5 était joignable et `mcw devices --json` a listé `/dev/video0` et `/dev/video1`. `mcw status --json --include-ready` a rapporté l'audio `healthy`, la vidéo `degraded` (des FD `/proc` inaccessibles), sans accès observé ; `doctor` a émis `warning` pour la vidéo, `watch --json` s'est arrêté proprement par SIGINT. Une connexion PipeWire volontairement invalide via `PIPEWIRE_REMOTE` a donné `Unavailable` et des erreurs dans `doctor`, sans interrompre le serveur.
- Une source virtuelle temporaire créée avec `pw-loopback` et consommée par `pw-record` a vérifié sur `serveur-asus` : absence d'accès → `Active` avec PID authentifié du recorder → arrêt ; `watch` a émis START et STOP pour ce PID. Les processus temporaires ont été arrêtés. Aucun micro matériel n'est exposé, la caméra `/dev/video0` reste inaccessible ; transitions physiques et redémarrage réel de PipeWire restent non vérifiés.
- `src/platform/macos.rs` et `native/macos_capture.swift` utilisent CoreAudio et AVFoundation sans capture intrusive : identité micro vérifiée via `libproc`, caméra toujours `pid: null`, application inconnue, décision `Unknown`, santé caméra `Degraded`. Le helper embarqué a réellement compilé et tourné sur les runners macOS 15 Apple Silicon et Intel ; 23 tests macOS et 28 tests Linux natifs ont réussi. Windows : 65 tests locaux, CI complète avec audit et MSI réussie. XDG/HOME et verrouillage historique restent portables ; le tray Windows a été réparé séparément.

## Ordre de validation avant publication

1. Préserver la CLI commune, le schéma JSON, les tests Windows, le backend Linux existant et les refus des actions Unix non implémentées. Ne pas remplacer `Unavailable` par un instantané vide sain ; ne pas fabriquer de PID caméra, de blocage, de mute ou de notification macOS.
2. Maintenir la CI multi-OS verte ; build et smoke macOS 15 Apple Silicon/Intel ont réussi. La cible minimale 15.0 du helper ne constitue pas un test de chaque version mineure. Ne pas contourner les erreurs SDK en ignorant le helper ou en simulant une observation.
3. Compléter les scénarios matériels macOS : API/TCC refusé, périphériques absents ou déconnectés et flux réels. Les commandes `status`, `devices`, `doctor`, `watch` et SIGINT sont déjà vérifiées sur runner sans matériel. Une panne doit donner `Degraded`/`Unavailable`, pas un faux inactif ; caméra toujours sans PID ni application attribuée, `--kill-unauthorized` refusé.
4. Exécuter tests et scénarios macOS sur un vrai Mac avec micro et caméra si accessible : repos → capture → arrêt, app distincte pour la caméra, autorisation refusée, déconnexion et reprise. Vérifier `watch` et `doctor` pendant chaque transition. Un runner CI sans périphérique prouve build/API/erreurs, **pas** la détection matérielle. Sans Mac équipé, documenter exactement les transitions non observées et conserver le statut expérimental des paquets macOS.
5. Conserver le reliquat Linux : sur un hôte autorisé à utiliser les périphériques, vérifier flux micro/caméra réels, PID, arrêt, capture ouverte mais inactive, redémarrage PipeWire, débranchement et permissions. Sur `serveur-asus`, le compte SSH n'a pas accès à `/dev/video0` et aucun `Audio/Source` matériel n'a été observé : ne pas transformer les tests synthétiques en preuve matérielle ni modifier les permissions sans accord. Aucune nouvelle fonctionnalité Linux n'est demandée par défaut.

## Invariants Linux à conserver

- Audio `Active` : flux PipeWire et source `running` reliés par lien `active`, identité PID vérifiée dans `/proc`. Vidéo `Active` : même preuve de graphe ; un FD `/dev/video*` ouvert n'est qu'un signal `Ready`/faible confiance et la possibilité d'un contournement V4L2 dégrade la couverture.
- Serveur PipeWire indisponible ou permissions `/proc` limitées : santé explicite, jamais vide et sain. `watch` n'invente pas de STOP à la perte du collecteur ; SIGINT ferme proprement. La CI Ubuntu compile, teste, vérifie Clippy et construit en release, mais ne remplace pas les scénarios sur matériel.

## Backend macOS : contrat et critères d'acceptation

- **Microphone, macOS 15+** : consulter et vérifier dans le SDK cible `AudioHardwareSystem.processes`, `AudioHardwareProcess.isRunningInput` et `pid`. `isRunningInput == true` prouve un flux d'entrée pour le processus exposé par CoreAudio ; rapprocher le PID d'une identité d'instance fiable avant toute attribution stable. Si cette identité ou la propriété manque, conserver l'activité observée sans inventer d'instance, et dégrader la santé selon la perte de couverture. Un périphérique présent ou une permission TCC ne prouve pas un flux.
- **Caméra** : `AVCaptureDevice.isInUseByAnotherApplication` signale l'utilisation par **une autre** application, sans identité de celle-ci. Si la propriété vaut `true`, l'observation par périphérique conserve `pid: null`, application inconnue, `EnforcementDecision::Unknown` et preuve de l'API ; le collecteur reste `Degraded` puisque l'usage par l'application elle-même et la couverture d'un environnement non interactif ne sont pas confirmés. Vérifier ce signal sur caméra physique, caméra virtuelle et partage multi-apps avant de revendiquer une détection matérielle. Ne jamais ouvrir la caméra pour fabriquer de l'activité ou contourner TCC.
- **Intégration** : `PlatformMonitor` implémente `CaptureCollector`, fournit inventaire et diagnostic et alimente le watcher Unix sans STOP mensonger lors d'une panne. TUI, tray, autostart, notifications, contrôle matériel et updater macOS restent désactivés. Tests, compilation helper et smoke CLI natifs ont réussi sur le runner macOS ; les transitions physiques restent à vérifier.
- Sources Apple consultées : [processes](https://developer.apple.com/documentation/coreaudio/audiohardwaresystem/processes), [isRunningInput](https://developer.apple.com/documentation/coreaudio/audiohardwareprocess/isrunninginput), [pid](https://developer.apple.com/documentation/coreaudio/audiohardwareprocess/pid), [isInUseByAnotherApplication](https://developer.apple.com/documentation/avfoundation/avcapturedevice/isinusebyanotherapplication). Vérifier les disponibilités dans le SDK utilisé.

## Livraison de la phase 3

- Modifications ciblées du backend et du watcher macOS, tests comportementaux pertinents ; Windows et Linux sans régression. Exécuter formatage, Clippy, tests, build et scénarios CLI sur chaque OS accessible ; distinguer compilation croisée, CI réelle et essai matériel.
- Mettre à jour README et `docs/ARCHITECTURE.md` avec une matrice par OS **et par ressource** : `Active`, `Ready`, PID, santé, permissions, notifications, tray et limites de version. Ne pas présenter le support macOS ou Linux matériel comme validé sans les observations correspondantes.
- Rapport final : commandes et résultats observés, SDK et version macOS, modèle des appareils, transitions réellement constatées, refus TCC, couverture CI, limites et prérequis restants. Ne pas publier de binaire macOS sur la seule base d'un build. Android reste hors périmètre ; `mcw update` reste désactivé sur Unix, où les archives natives s'installent manuellement après vérification de `SHA256SUMS`.

## Preuves et décisions de portée retenues pour 0.15.0

- Portées explicitement approuvées : Linux mute/restauration des sources de
  session PipeWire, sans déni global ALSA/V4L2 ; macOS mute INPUT uniquement
  lorsqu'il est inscriptible et profil caméra possédé approuvé manuellement.
  Aucune promesse de blocage universel du micro macOS. Observation du
  verrouillage macOS par API publiques seulement : Unknown, enable refusé.
- Canal de publication approuvé : `v0.15.0` Unix en prerelease, jamais stable
  latest sans Windows. Installer avec `--version v0.15.0` ; `v0.14.0`, ses
  artefacts Windows et les consommateurs `/releases/latest` restent préservés.
  Publication/installation Windows nouvelle retenue ; aucun contournement
  Defender, rétablissement de quarantaine, certificat fictif ou verdict AV.
- [Dry run natif 37270375386](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37270375386)
  réussi : Ubuntu 22.04/PipeWire 0.3.48, macOS 15.7.9 ARM/Intel, SDK 15.5,
  Swift 6.1.2 en mode Swift 5, cible 15.0 ; format/Clippy/tests/builds,
  CLI extraite, sept langues et aides directes, TUI, vrais tray/menu bar,
  autostart possédé, son accepté par API, bundle SPDX et provenance.
  Linux : notification visible et START schema-3 livré à journald ; panne
  PipeWire réelle sans faux START/STOP. macOS : vrai NSSound et LaunchAgent.
- [CI Windows 37267498847](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37267498847) :
  99 tests, audit et MSI natif per-user install/uninstall réussis en runner.
  Sur le poste Windows, capture C270 temporaire observée hors ktalk :
  121 frames jetées, Camera Active, PID inconnu conservé, START/STOP ;
  aucune conservation média. Mise à jour du PE CLI mappé : deux renames
  récupérables avec bref trou de pathname, tray remplacé atomiquement.
- `serveur-asus` réel, PipeWire 1.0.5 : PID Client authentifié, source virtuelle
  active, mute False→True→False confirmé indépendamment, source prémutée
  préservée, sélection TUI puis K/Esc/Q sans terminaison, STOPPED humain
  explicitement « dernière observation ». Fixtures et processus possédés retirés.
- Limites non revendiquées : MacBook M1 Pro/macOS 27 sans accès distant ni
  runner configuré ; capture physique Linux/macOS, effet matériel du mute
  INPUT/profil caméra, transitions lock/unlock, autorisation notification
  macOS et audibilité des haut-parleurs. Les runners sans matériel ne
  prouvent pas ces scénarios. L'attribution INPUT Sound/Telegram reste
  inconnue : les devices OUTPUT CoreAudio ne la prouvent pas.
- Les modifications utilisateur du TUI et de ce handoff sont préservées ;
  `WATCHDOG.yml` n'est pas inclus dans les commits de cette livraison.
### Canal final approuvé : v0.15.1

Le tag nouveau `v0.15.0` reste immuable et non publié. Son smoke associait
incorrectement un scan complet dégradé de shutdown à une période indisponible.
Le test distingue maintenant cette frontière, conformément aux invariants
ci-dessus ; aucun changement de production n'est masqué. L'utilisateur a choisi
la nouvelle prerelease Unix `v0.15.1` (`--version v0.15.1`) plutôt que déplacer
le tag. Stable/latest `v0.14.0` et les anciens artefacts restent inchangés.

