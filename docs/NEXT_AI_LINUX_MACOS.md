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

### Publication et vérification finales de v0.15.1

- Prerelease Unix publiée :
  https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.15.1
  via le run natif vert `37299861471`. Trois archives Unix, SPDX et SHA256SUMS ;
  aucun nouveau ZIP/MSI Windows. API `latest` reste `v0.14.0` ; tag `v0.15.0`
  inchangé, sans release/artefacts.
- Migration publique réelle `v0.14.0` → `v0.15.1` verte sur Linux x64,
  macOS ARM et Intel, avec bash/zsh/fish :
  https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37301377449
  PATH interactif/login et consentement, refus d'exécutable actif, échecs sans
  corruption, préférences/historique et désinstallation possédée exercés.
- Trois archives et SPDX vérifiés par SHA256SUMS/digests API et attestations
  du workflow release, commit exact et ref `refs/tags/v0.15.1`. Le manifeste
  SHA256SUMS n'est pas lui-même attesté.
- SSH réel : upgrade bash, nouveaux shells et désinstallation dans un préfixe
  temporaire privé ; `mcw update` garde `0.15.1`, ne rétrograde pas vers stable
  `0.14.0`. Bannière système bash laissée intacte ; aucun paquet zsh/fish ajouté.
- CI Windows du commit publié verte : 99 tests, audit et MSI per-user natif,
  https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37299189617
  sans publication/installation nouvelle sur le poste utilisateur ni verdict AV.
- Le smoke ne fige plus les diagnostics doctor sur la liste des collecteurs ;
  ses vrais contrôles JSON et lifecycle restent exercés. Les limites matérielles
  macOS 27/INPUT/caméra/profil/TCC/verrouillage/audio restent celles ci-dessus.

## État final avant nouvelles fonctions — source 0.16.0

Ce bloc décrit l'état courant et supplante les limites historiques du footer
ci-dessus. L'implémentation accessible est terminée et exercée ; les seuls
scénarios non certifiés sont les effets physiques/sessions explicités plus bas.
Ne pas les convertir en succès parce qu'un runner n'a pas le matériel.
Les notes et travaux utilisateur précédant ce footer restent préservés.

### Source, vérifications et canaux

- Branche : `work/native-parity-20261003`. Code vérifié :
  `8be1f1b78a907fbeaa47c60e3995369974212588`.
- [Dry run natif 37429480145](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37429480145)
  vert sur Linux x64, macOS Apple Silicon et Intel : format, Clippy strict,
  tests, builds debug/release, véritables CLI/PTY/tray/menu bar, lifecycle,
  bundle SPDX/SHA256SUMS et attestations. Linux : 74 tests bibliothèque + 1 CLI,
  contrôle micro virtuel, vrais menu/notification inspectés visuellement,
  restauration après SIGTERM/SIGKILL et frontières root/utilisateur vérifiées.
- [CI complète 37429482692](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37429482692)
  verte sur ce même code, y compris Windows : tests, audit Rust, build release
  et installation/désinstallation MSI per-user natifs.
- [Migration publique 37422717465](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37422717465)
  verte : `v0.14.0` → `v0.15.1`, bash/zsh/fish, Linux x64 et deux architectures
  macOS, consentement/PATH, erreurs sans corruption et désinstallation possédée.
- Canal maintenant approuvé par l'utilisateur : `v0.16.0` stable **latest** sur
  `main`, avec ZIP/MSI Windows et trois archives Unix. `v0.14.0`, `v0.15.1` et
  le tag `v0.15.0` restent immuables ; aucune ancienne release n'est remplacée.
  `WINDOWS_RELEASE_APPROVED=true` autorise cette publication complète, pas un
  certificat de signature ni un verdict antivirus. La preuve publique de
  publication/migration est consignée séparément après génération des artefacts.
- Windows local reste la paire réelle CLI/tray `0.15.1`, issue de la source
  publiée `7d1e3dc17a6171b521a2f53b3cfebe663425ceb0` : CLI/TUI/tray/update
  exercés, autostart possédé préservé, scans Defender réels terminés aux chemins
  installés avec protections actives. Pas de signature/certificat inventé,
  exclusion, restauration de quarantaine ni verdict AV universel.
  Voir [SIGNING.md](SIGNING.md) pour les digests et limites exactes.

### Caméra USB Linux : validation matérielle terminée

- `serveur-asus`, UID 1000, noyau `7.0.0-28-generic`, PipeWire `1.0.5` :
  webcam USB `13d3:5a11`, configuration unique, interfaces `1-6:1.0`/`1-6:1.1`
  initialement `uvcvideo`.
- Archive de source `e4886c345c656c430c589893fceec95cf2838d6e` réellement
  vérifiée par SHA-256 et provenance GitHub, puis re-vérifiée après transfert :
  `2fb1dbabeb5f297317c3fc99c32164b2cc3485646573a3f40b58d07541e4b86a`.
- Installation root explicitement revue, aucune action caméra implicite.
  Annulation Polkit : tous les bindings et nœuds vidéo inchangés.
- CLI non-root, véritable agent Polkit utilisateur enregistré et mots de passe
  administrateur frais : block révoque une capture FFmpeg déjà active
  (`VIDIOC_DQBUF: No such device`), supprime les deux nœuds vidéo et refuse
  une nouvelle capture même en root. Bluetooth, Ethernet et hubs inchangés.
- Allow avec nouvelle approbation Polkit restaure exactement les deux drivers
  originaux et les nœuds ; cinq images YUYV 640x480 capturées à nouveau vers
  `null`. Aucun média conservé. Un cycle sudo séparé vérifie aussi block répété
  sans adoption d'un état extérieur.
- L'agent interne de `pkexec` échoue sur cet hôte avec `No session for cookie`,
  également hors MCW ; redémarrer le daemon n'a pas corrigé ce défaut.
  L'agent standard `pkttyagent` non privilégié enregistré pour le PID CLI
  fonctionne. Aucune règle d'autorisation, PAM, permission ou compte assoupli.
  Un agent fonctionnel est un prérequis, pas un contournement de Polkit.
- Après allow : journal vide, retrait root possédé réussi, helper/policy/receipt
  et cache/journal vides retirés, inode du verrou permanent préservé.
  Candidats privés, configuration isolée et agent temporaire retirés.
  Aucun helper expérimental laissé installé ; bindings physiques restaurés.

### Mute PipeWire : défaut systemd corrigé et vérifié

- Le socket activé par systemd expose le PID du gestionnaire utilisateur
  (`1021`), pas l'exécutable PipeWire. L'ancien code échouait sur son
  `/proc/1021/exe` inaccessible.
- `procfs::verify_peer_instance` vérifie UID et génération PID/starttime sans
  exiger cet exécutable ; boot/socket/core cookie/node serial restent épinglés.
  L'identité complète des processus de capture n'est pas relâchée.
  Régression permanente avec vrai processus non-dumpable et refus du mauvais UID.
- Archive corrigée `8be1f1b`, SHA-256
  `df7c4700ad34d1c141dbdf806c044f15972de511f80460d48b9b82a55383d966`,
  provenance et transfert vérifiés. Sur le socket réel : doctor passe de
  l'erreur à exit 0, source virtuelle silencieuse False→True→False confirmée
  indépendamment par `pw-dump`, original prémuté maintenu après restore.
  Zéro état original retenu et zéro restauration en attente ; source retirée.
  Cela ne prouve pas un mute physique, ni un déni global ALSA.

### Correction macOS et crédit public

**Thank you [@repentandliveholy](https://github.com/repentandliveholy) for the
real-device diagnosis, public CoreMediaIO fix and local validation.**

- Telegram/macOS 27 : AVFoundation restait false, CoreMediaIO 0→1→0 et
  FaceTime HD Camera START/STOP dans le build corrigé de l'ami : preuve rapportée
  par l'ami, pas un scénario rejoué par le mainteneur.
- Source `0.16.0` : inventaire AVFoundation passif, activité CoreMediaIO publique,
  état nullable/unknown conservé, aucune fausse inactivité ni faux STOP ;
  PID/client inconnus, confiance medium, couverture dégradée, aucune frame lue
  et aucune attribution/enforcement par application inventée.
- Builds et surfaces macOS natifs exercés ; preuve observée macOS `15.7.9`,
  SDK `15.5`, ARM/Intel. Inventaires système et MCW séparés : zéro caméra sur
  runners ; ce n'est pas une réfutation du défaut matériel de l'ami.
  [PR #2](https://github.com/Roman-Cuisset/miccamwatch/pull/2) conserve le crédit
  anglais correct. Les fichiers bruts `debug_macos/*` ne sont pas publiés.

### Portées finales et seules preuves restant inaccessibles

| Contrôle | Linux | macOS |
| --- | --- | --- |
| Micro | Mute/restauration des sources de la session PipeWire ; pas de déni ALSA/autres sessions. | INPUT uniquement lorsqu'il est inscriptible ; pas de blocage universel. |
| Caméra | USB/UVC conforme, autorisation explicite, restauration possédée ; cycle physique vérifié ci-dessus. | Profil Restrictions possédé à approuver/retirer manuellement ; effet matériel non exercé ici. |
| Verrouillage | Session graphique logind locale nécessaire ; aucun block caméra automatique au lock. | État public Unknown, activation lock-policy refusée ; aucun mécanisme privé ajouté. |
| Attribution | PID PipeWire authentifié avec identité complète quand observable ; V4L2 direct reste limité. | Activité caméra par device, client/PID unknown ; identité INPUT non garantie pour Sound/Telegram. |

- Linux : aucun microphone physique exporté ; seule session seat0 observée =
  greeter lightdm, pas une session graphique utilisateur testable. Mute physique,
  vrais lock/unlock et audibilité ne sont pas certifiables depuis cet accès SSH.
- MacBook M1 Pro/macOS 27 : pas d'accès Mac SSH ni runner auto-hébergé fourni.
  INPUT physique, caméra/profil approuvé, autorisation notifications/TCC et
  audibilité restent non exercés. Actions natif couvre le logiciel, pas ce matériel.
- Aucun développement accessible ni scaffold n'est laissé à terminer.
  Les nouvelles fonctions peuvent partir de cette base testée, dans ces portées ;
  ne pas transformer les limites ci-dessus en promesse de parité universelle.
- Travaux utilisateur TUI/handoff et `WATCHDOG.yml` préservés. `secrets.env`
  exclu par `/secrets.env` dans `.gitignore`, non suivi ; mot de passe jamais
  affiché, mis en argument de commande ou committé.

## Publication stable latest v0.16.0 — 2026-10-06

- Autorisation utilisateur ultérieure : fusionner dans `main` et publier
  `v0.16.0` stable/latest, Windows compris. PR #2 fusionnée :
  `0e7cc9c97d6cb926ad61ab2b26a25ae85096638a`.
- Tag immuable `v0.16.0`, source `9515fb664ac0c75601ebf6b8321b6954666faec9`.
  [CI main 37435307905](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37435307905)
  verte sur Windows/Linux/macOS ARM/Intel avant création du tag.
- [Release 37437092551](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37437092551)
  verte ; [release publique](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.16.0)
  publiée à `2026-10-06T08:42:55Z`. API latest : ID `404493096`,
  `draft=false`, `prerelease=false`, sept assets complets.
- ZIP/MSI Windows, trois archives Unix, SPDX, SHA256SUMS téléchargés et vérifiés
  contre les digests API ; six payloads/SBOM également contre le manifeste et
  les attestations GitHub, workflow release/source/ref/runner hébergé exacts.
  Le manifeste n'est pas séparément attesté.
- Migrations publiques par l'installateur courant, bash/zsh/fish et trois
  architectures : depuis [v0.14.0](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37438076601)
  et [v0.15.1](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37438076504),
  toutes vertes. Default du workflow natif désormais `v0.16.0`.
- Sur Linux SSH réel, installation sans `--version` résout bien latest 0.16.0.
  **Limite de migration :** `mcw update` Linux 0.15.1 embarque l'ancien
  installateur à trois fichiers et refuse le nouveau paquet à six fichiers.
  Refus exercé : CLI 0.15.1 préservée. Relancer l'installateur public courant
  avec le même préfixe migre réellement vers 0.16.0 ; le nouvel updater
  confirme ensuite up-to-date. Ne pas prétendre que l'ancien updater migre seul.
- Windows : véritable updater 0.15.1 → 0.16.0 dans un préfixe portable privé,
  deux EXE identiques au ZIP public ; second appel up-to-date, valeurs HKCU
  autostart inchangées. Pendant la vérification, la paire réellement installée
  a été observée en 0.16.0, avec les deux hashes du ZIP public. L'origine de
  cette mise à jour concurrente n'est pas observée ; ne pas l'attribuer au smoke
  privé. Le nettoyage n'a restauré/remplacé aucun fichier installé utilisateur.
  ZIP/tray et MSI install/remove natifs également exercés dans Actions.
- Aucun certificat configuré : les deux EXE et le MSI sont `NotSigned`.
  Defender actif, signature `1.459.568.0`, scan privé exact 1000/1001 terminé,
  zéro événement 1116/1117 dans l'intervalle observé ; pas un verdict Microsoft
  ni une garantie AV universelle. Aucun contournement de protection.
- Anciennes releases/tags immuables ; aucun secret, debug brut ou travail local
  utilisateur publié. Les limites matérielles précédentes restent inchangées.

