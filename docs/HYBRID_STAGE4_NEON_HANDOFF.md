# MicCamWatch v0.22.0 — revue indépendante et transfert étape 4 (Gemini 4 Neon)

Date : **2026-11-25**  
Statut étape 4/6 : **EN COURS / NON CLÔTURÉE** (aucun passage à l'étape 5, aucune fusion, aucun push, aucune publication).

Ce document permet à GPT-6.2 Sol (Codex) de reprendre directement les résultats de cette revue indépendante et les corrections non sensibles vérifiées sans refaire l'audit ni perdre la traçabilité.

---

## 1. Base Git exacte et isolation

| Élément | Valeur vérifiée |
| --- | --- |
| Branche de référence (intacte) | `work/flutter-hybrid-0220` |
| HEAD de référence vérifié | `16d301245626cd63744c676aa64dbfd1b1be62be` |
| Base précédant les 14 commits étape 4 | `e773cbf266cc25471a627265d6379cf627ebcfa5` |
| Branche dédiée créée | `work/stage4-neon-review` |
| Worktree isolé créé | `C:/Users/stcui/Documents/miccamwatch/target/stage4-neon-review` |
| `main` (intacte) | `73369abda09bdadce8c779a9559b3e2334a32ecb` |
| Tag `v0.21.0` (intact) | `06d800ba536b5e6945abe4a4345957ba85c79cbf` (commit cible `3d74686ace7b04728794765f9eb57c0c0a8bc84c`) |
| Stash (intact) | `stash@{0}` sur `work/native-parity-20261003` |
| Installation hôte v0.21.0 | Non touchée (aucune installation, désinstallation, mutation MSI ou action matériel) |

---

## 2. Commits de l'étape 4 examinés (14 commits : `e773cbf..16d3012`)

1. `6ad3a58` — `Validate macOS termination authority before acting` (`native/macos_process.swift`)
2. `c5628e1` — `Report Unix observation gaps and macOS notification settings` (`native/macos_capture.swift`, `native/macos_desktop.swift`, `src/frontends/cli.rs`, `src/main.rs`, `src/notify/unix.rs`, `src/output.rs`, `src/platform/macos.rs`, `src/platform/unix.rs`, `src/unix_notify.rs`, `src/watcher/unix.rs`, workflow)
3. `b3350bc` — `Fit Flutter windows and explain platform profile behavior` (`desktop/flutter/lib/{main,preferences}.dart`, `desktop/flutter/test/preferences_test.dart`, runners Linux/macOS/Windows)
4. `73d626f` — `Scan paged history records in bounded blocks` (`src/history/reader.rs`)
5. `6274381` — `Localize macOS notification states and qualify native refusals` (`desktop/flutter/lib/{consultation_panels,labels,main}.dart`, `desktop/flutter/test/consultation_test.dart`, `installer/tests/macos-hybrid-installed.py`, `src/{output,platform/macos,watcher/unix}.rs`, workflow)
6. `b60fc3b` — `Sample installed stable and hybrid Windows desktop resources` (`installer/tests/windows-hybrid-install.ps1`, workflow)
7. `7252421` — `Record stage 4 audit and first qualification evidence` (`docs/*`, `src/platform/macos.rs`)
8. `8e5daa2` — `Rebaseline Windows terminal events after collection gaps` (`src/frontends/tui.rs`, `src/notify/capture.rs`, `docs/HYBRID_STAGE4_AUDIT.md`)
9. `13705fa` — `Isolate Linux Flutter renderer CPU on the pinned hosted build` (`desktop/qualification/{linux,linux_hosted,replay_products}.py`, workflows, docs)
10. `c4d3b3b` — `Keep Unix observation independent of notification delivery` (`src/watcher/unix.rs`, `src/i18n.rs`, workflow, docs)
11. `199436c` — `Use Skia with the explicit Linux software compositor` (`desktop/flutter/linux/runner/my_application.cc`, workflow, docs)
12. `ab52750` — `Reuse verified Windows MSI bytes for paired desktop measurements` (`installer/tests/windows-hybrid-install.ps1`, workflows)
13. `7ac58f5` — `Retain both Linux renderer receipts and track thread-owned windows` (`desktop/qualification/{linux,linux_hosted,replay_products}.py`, workflow, docs)
14. `16d3012` — `Record stage 4 native qualification and performance receipts` (`docs/HYBRID_STAGE4_AUDIT.md`)

---

## 3. Résultats de la revue indépendante

### 3.1 Phase A — Audit objectif des 14 commits

- **Sécurité et autorité (`6ad3a58`, `6274381`, `c4d3b3b`) : aucun affaiblissement constaté.**
  - `native/macos_process.swift` valide désormais le compte ordinaire (`uid != 0 && uid == geteuid()`), la naissance/exécutable (`libproc`), les trois UID (`pbi_uid`, `pbi_ruid`, `pbi_svuid`), `PROC_FLAG_SYSTEM == 0` et la liste des exécutables protégés **avant** `task_for_pid`, puis revalide `audit_token_t` (PID, pidversion, ruid, euid, `asid` identique à l'appelant), `pid_for_task` et une seconde lecture `libproc` avant `task_terminate`. Aucun repli vers `kill(pid)`, aucune élévation, aucun contournement SIP/Hardened Runtime.
  - `src/platform/macos.rs` distingue honnêtement `CollectorState::Unavailable` (aucune caméra ou aucun état `running_somewhere` connu) de `CollectorState::Degraded` (au moins un état `running_somewhere` lu, avec limitation d'attribution/flux ou erreur partielle). L'observation partielle préserve les accès actifs sans inventer `STOP` (`observation_gaps.push(Resource::Camera)`).
  - `src/watcher/unix.rs` (`c4d3b3b`) : `EventDispatcher` est bien réutilisé aussi par `src/frontends/tray/unix.rs:2362`, donc `mcw watch` **et** `mcw tray` sur Linux/macOS bénéficient de la file bornée à 32 alertes non bloquante avec relecture de `Settings::load()` avant livraison.
  - `src/history/reader.rs` (`73d626f`) : le scanner par blocs inverse conserve exactement les bornes `MAX_LINE_BYTES` (64 Kio) et `PAGE_SCAN_BYTES` (512 Kio), sans risque de débordement d'entier (`span.len() <= min(self.buffered, *budget)` ; si `newline.is_some()`, `span.len() + 1 <= remaining.len()`). Le gain mesuré (~2×) ne réduit aucune vérification ni pagination.
  - `src/frontends/tui.rs` (`8e5daa2`) : le rebasage par ressource (`event_coverage: [bool; 2]`) exige deux scans consécutifs complets pour la ressource concernée avant d'émettre `START`/`STOP` après un trou ou une erreur de collecte, et `current_accesses` est mis à jour sur chaque scan.
  - `desktop/flutter/lib/consultation_panels.dart` (`6274381`) : `l[check.name]` retombe proprement sur `check.name` via `Labels.operator[]` lorsque le nom de contrôle technique n'est pas traduit (pas de plantage), tout en localisant `notification_authorization` et en séparant systématiquement l'état d'autorisation de `macosBannerUnverified`.
- **Défauts / régressions découverts dans les changements récents ou le code Flutter existant :** détaillés en sections 4 et 5 ci-dessous.

### 3.2 Phase B — Vérification des performances (Linux et Windows)

#### Linux
- **Conditionnement Skia (`199436c`, `desktop/flutter/linux/runner/my_application.cc:103-105`) :**
  - `fl_dart_project_set_enable_impeller(project, FALSE)` n'est appelé **que** si `g_strcmp0(g_getenv("FLUTTER_LINUX_RENDERER"), "software") == 0`.
  - Le renderer par défaut sur environnements GPU/Wayland normaux reste strictement inchangé.
  - Les erreurs de démarrage ne sont pas masquées (`g_application_register` retourne `exit_status = 1` en cas d'échec).
  - `desktop/qualification/linux.py` (`7ac58f5`) corrige un vrai défaut de comptabilité de processus : la lecture des enfants sur tous les `/proc/<pid>/task/*/children` avec déduplication évite de manquer un enfant Flutter lancé depuis un thread secondaire.
  - Les reçus CI montrent la disparition effective du processus Flutter à la fermeture (retour à 1 processus Rust, 9,88–10,03 Mio RSS, 0,27–0,33 % d'un cœur).
  - **Cause applicative supplémentaire identifiée côté Flutter (voir §4, F2/F3) :** `SnapshotSession.tick()` et `_MicCamWatchAppState.build()` reconstruisaient inutilement tout l'arbre `MaterialApp` et recalculaient `ColorScheme.fromSeed(...)` chaque seconde au repos même sans changement d'état, ce qui amplifie le coût CPU sous rendu logiciel et génère des allocations inutiles sur Windows comme sur Linux.

#### Windows
- **Revue de la méthode de mesure (`installer/tests/windows-hybrid-install.ps1:364-441,513-608`) :**
  - Vérifie l'empreinte SHA-256 exacte de `v0.21.0` et du MSI candidat, ainsi que la couverture des collecteurs (`mcw.exe status --json`) avant et après chaque intervalle de 15 s (16 échantillons espacés d'1 s).
  - Sépare correctement `WorkingSet64` (RSS) et `PrivateMemorySize64` (commit privé), et somme exactement sur les handles de processus suivis (`@($launch)` fermé, `@($launch, $ui[0])` ouvert).
  - **Limites méthodologiques déjà connues et confirmées :**
    1. Un seul tour court de 15 s sans warmup de 30 s ni alternance AB/BA (contrairement au protocole cible de `RELEASE_0.22.0_PLAN.md` §7 : 3 paires AB/BA, 180 s).
    2. Points de départ asymétriques pour la latence d'ouverture : `baselineWindowVisibleMs` (298 ms) mesure `mcw.exe tray open` sur un tray **déjà actif**, alors que `candidateWindowVisibleAndIpcMs` (1 105 ms) mesure depuis le lancement à froid de `mcw-tray.exe --open` jusqu'à fenêtre visible + acquittement IPC.
    3. Runner hébergé sans capture active (`privacy_store` et `module_scanner` indisponibles dans les deux versions) : ne représente pas la charge sous micro/caméra actifs ni une garantie universelle sur tous les PC.
    4. Les processus système hors arbre (`dwm.exe`) et helpers éphémères ne sont pas inclus (explicitement mentionné dans `processAccounting`).
  - **Surcoût RAM fenêtre Flutter (96,14 Mio RSS / 63,82 Mio commit privé vs 25,18 Mio / 4,08 Mio Win32 natif) :** attendu par la présence du moteur Flutter + Skia + ICU/Dart VM tant que la fenêtre est ouverte ; la libération complète à la fermeture ramène Rust seul à 18,44 Mio RSS / 3,93 Mio commit privé (1 processus).

### 3.3 Phase C — Interface Flutter

Revue complète du code Dart (`desktop/flutter/lib/*.dart`) et des 3 runners natifs (`win32_window.cpp`, `flutter_window.cpp`, `my_application.cc`, `MainFlutterWindow.swift`) :
- **Contraste Palette (`main.dart:58-72`) :** vérifié sur les deux thèmes ; toutes les paires texte/surface (`text`, `secondary`, `accent`, `warning` sur `card`/`background`/`tint`) dépassent WCAG AA (≥ 5,0:1 en clair et ≥ 6,7:1 en sombre).
- **Runners natifs (`b3350bc`) :**
  - macOS (`MainFlutterWindow.swift`) : restaure le cadre (`MicCamWatchDesktopWindow`), borne taille et origine dans `visibleFrame` de l'écran actif (ou `NSScreen.main` si l'ancien écran a disparu), abaisse le minimum à `min(640, content.width) × min(440, content.height)`.
  - Linux (`my_application.cc`) : borne la taille initiale et minimale (`640×440`) sur `gdk_monitor_get_workarea` avec marge pour les décorations. La persistance géométrique Linux/Windows reste ouverte (documenté dans `HYBRID_STAGE4_AUDIT.md`).
  - Windows (`win32_window.cpp`, `flutter_window.cpp`) : borne la création et `WM_GETMINMAXINFO` (`640×440` mis à l'échelle DPI) sur `rcWork`. Note P2 : aucun gestionnaire `WM_DISPLAYCHANGE` si la résolution ou le moniteur change pendant que la fenêtre est ouverte.
- **Défauts reproductibles identifiés dans le shell et les panneaux Flutter :** listés en §4 (F1 à F7) et corrigés pour les défauts non sensibles en §6.

### 3.4 Phase D — Problèmes macOS encore ouverts (analyse code seul, sans modification sensible)

#### 1. Terminaison de Vorssaint (`native/macos_process.swift`, `src/platform/macos.rs`)
- **Cause racine technique confirmée par lecture du code et des API publiques macOS :**
  - `terminateObservedTask` s'appuie sur `task_for_pid(mach_task_self_, pid, &task)` suivi de `task_terminate(task)`.
  - Sous macOS moderne (SIP + Hardened Runtime / AMFI / `taskgated`), un processus non privilégié sans entitlement debugger (`com.apple.security.cs.debugger`) ne reçoit **jamais** de port Mach (`KERN_FAILURE`, Mach 5) pour une application utilisateur tierce signée/durcie (comme Vorssaint, UID 501), même sous le même UID.
  - Lancer avec `sudo` échoue désormais proprement **avant** `task_for_pid` avec le message explicite `run MicCamWatch as the target application's ordinary user, without sudo` (`6ad3a58`).
  - Le test `native_termination_rejects_parent_and_stale_observation` (`6274381`) qualifie uniquement deux refus (parent protégé et identité de naissance périmée sur `/bin/sleep`), pas la terminaison réussie d'un processus utilisateur durci.
- **Distinction claire des trois mécanismes (à soumettre à décision de sécurité avant toute implémentation) :**
  1. *Fermeture normale demandée à l'application (`NSRunningApplication.terminate()`)* : API publique AppKit envoyant une demande de fermeture coopérative à une app enregistrée dans LaunchServices (sans `task_for_pid`, sans contournement SIP). L'app peut refuser/différer (document non enregistré), et l'identité de l'instance doit être vérifiée (PID + `processIdentifier` + `launchDate` / `bundleURL` / `executableURL` corrélés à `libproc`) puis sa sortie observée séparément. Ne couvre pas les démons/CLI sans `NSRunningApplication`.
  2. *Terminaison forcée du processus* : `task_for_pid` + `task_terminate` est bloqué par macOS sur presque toutes les apps utilisateur durcies ; `NSRunningApplication.forceTerminate()` ou `kill(pid, SIGKILL)` exposent un risque TOCTOU de recyclage de PID en l'absence d'un appel noyau public par `audit_token` ou `pidfd` sur macOS.
  3. *Blocage du microphone* : la sourdine CoreAudio INPUT agit au niveau des propriétés audio modifiables, indépendamment de la terminaison d'un processus, et ne révoque pas TCC.
- **Aucune modification apportée aux gardes ou à la logique de terminaison** conformément à la consigne.

#### 2. Caméra CoreMediaIO (`native/macos_capture.swift`, `src/platform/macos.rs`)
- **États vérifiés dans le code et les tests (`src/platform/macos.rs:464-527,1035-1117`) :**
  - Active (`runningSomewhere == true`) → `Activity::Active`, `Confidence::Medium`, `application: "Unknown application"`, `pid: None`, `enforcement: Unknown`, collecteur `Degraded` (pas d'attribution inventée).
  - Inactive observée (`runningSomewhere == false`, pas d'erreur) → `accesses` vide, `observation_gaps` vide, collecteur `Degraded` (limitation d'attribution/flux conservée).
  - Application inconnue → toujours explicite (`"Unknown application"`, jamais éligible à la terminaison).
  - Indisponible (`devices: []`, ou tous `runningSomewhere == null`/absent) → `CollectorState::Unavailable` + `observation_gaps: [Camera]` (supprime tout `STOP` inventé).
  - Dégradée partielle (un périphérique lu + un périphérique en erreur/null) → préserve l'accès actif connu, `CollectorState::Degraded`, `observation_gaps: [Camera]`.
  - **Point ouvert :** reprise physique après veille/déconnexion caméra sur Mac réel et comportement de `AVCaptureDevice.devices(for: .video)` (API dépréciée au profit de `AVCaptureDevice.DiscoverySession` en amont) après veille restent non qualifiés sans matériel autorisé. Aucune modification du collecteur natif n'a été implantée.

#### 3. Notifications macOS (`native/macos_desktop.swift`, `src/watcher/unix.rs`, `src/platform/macos.rs`)
- **Points vérifiés :**
  - `notification-status` lit `UNUserNotificationCenter.getNotificationSettings` sans jamais demander l'autorisation ni envoyer d'alerte, distinguant `not_determined`, `denied`, `authorized`, `authorized_without_alerts`, `provisional`, `unknown`.
  - `banner_delivery=unverified` est toujours distinct de l'autorisation et de la soumission (`state: "submitted"`).
  - File Unix bornée à 32 alertes (`c4d3b3b`) : saturation et reprise testées (`stalled_alert_worker_never_blocks_the_dispatcher_queue`), non bloquante pour la collecte.
- **Limites et point mineur identifié :**
  - `src/platform/macos.rs:535-560` (`Helper::install()`) extrait toujours `MicCamWatchHelper.app` dans un dossier temporaire unique par processus (`mcw-<pid>-<ns>-<attempt>`) avec signature ad hoc : la persistance du consentement et l'affichage effectif d'une bannière après redémarrage restent non prouvés sur Mac réel.
  - Point mineur P3 dans `src/watcher/unix.rs` (`deliver_alerts`, `c4d3b3b`) : `if sound` ne revérifie pas `!stopped.load(Ordering::Acquire)` après `Settings::load()` avant d'appeler `platform::play_chime()` (qui peut attendre jusqu'à 10 s sur macOS), contrairement à `if notify && !stopped.load(Ordering::Acquire)`. Non modifié dans ce lot (hors zone Windows testable ici).

### 3.5 Phase E — Windows et maintenance MSI (lecture seule + tests unitaires non privilégiés)

- **Documents et scripts revus :** `docs/WINDOWS_MSI_MAINTENANCE.md`, `docs/WINDOWS_MSI_ROLLBACK_FORENSICS.md`, `docs/HYBRID_DISTRIBUTION.md`, `docs/WINDOWS_STANDARD_USER.md`, `installer/tests/windows-hybrid-install.ps1`, `installer/tests/windows-hybrid-standard.ps1`.
- **Aucune modification du code MSI, du pilote de maintenance, de Hyper-V, des checkpoints ou du PC principal.**
- **Qualification MSI en compte standard sur révision récente (`16d3012` / post-`e74a86d`) : EXPLICITEMENT OUVERTE.**
  - La CI hébergée x64 renvoie `1625` (`DisableMsi=1`) dès l'installation de `v0.21.0`.
  - La qualification VM `MCW-Win11-x64-Lab` reste attachée à `e74a86d` (étape 3) et n'a pas été rejouée sans autorisation dédiée.
- **Scénarios de non-régression préparés pour la reprise dans `MCW-Win11-x64-Lab` (compte standard) :**
  1. Témoin `v0.21.0` → upgrade MSI candidat étape 4 → vérification manifeste 16 fichiers + IPC 10 + fermeture Flutter avec tray actif + conservation données/autostart + désinstallation.
  2. Panne injectée après `RemoveExistingProducts` (`v0.21.0` → candidat et candidat → futur) → vérification détection `State=1` / `1605` par `recover-update` → refus UAC (`1602`, `msi_cancelled`) → reprise interactive (`restore` → `remove` → redémarrage VM → `reinstall`) → validation index `UpgradeCodes` et cache MSI.
  3. Vérification en lecture seule qu'aucun lancement élevé du pilote n'est accepté et qu'un mutex MSI concurrent bloque la récupération.

---

## 4. Défauts trouvés et 5. Gravité (P0/P1/P2/P3)

| ID | Gravité | Composant / Origine | Constat reproduit | Statut dans cette branche |
| --- | :---: | --- | --- | --- |
| **F1** | **P1** | `desktop/flutter/lib/preferences.dart:342-346` (introduit par `b3350bc`) | `profileEffect` lisait `values.profile` (choix **enregistré**) au lieu de `view.appliedProfile` (profil **appliqué** par le moteur). Sous `--config` explicite (`appliedProfile == null`) ou quand enregistré ≠ appliqué, Windows affichait faussement l'effet du profil enregistré (ex. Strict) comme actif. Le test de `b3350bc` ne couvrait que `profile == applied`. | **Corrigé et testé** (`f50b477`) |
| **F2** | **P1** | `desktop/flutter/lib/main.dart:353-356` + `ObservationView` | `_panel` utilisait un `Container` opaque (`DecoratedBox` avec `color: c.card`) entourant `ObservationView` qui contient un `SwitchListTile`. En mode debug/test, Flutter lève l'assertion `ListTile background color or ink splashes may be invisible`, et en release le surlignage de focus clavier/survol du `SwitchListTile` est masqué derrière la boîte opaque. | **Corrigé et testé** (`a256748`) |
| **F3** | **P1/P2** | `desktop/flutter/lib/main.dart:124,243-252`, `snapshot.dart:265-271`, `commands.dart:116`, `preferences.dart:97`, `consultation.dart:152` | Chaque seconde au repos, `SnapshotSession.tick()` appelait `notifyListeners()` sans changement d'état, déclenchant `setState` sur toute l'app, recalculant `ColorScheme.fromSeed` + `ThemeData` à chaque build et déclenchant 3 cascades `notifyListeners()` redondantes même à séquence et connectivité inchangées. | **Corrigé et testé** (`22ffd98`) |
| **F4** | **P1** | `desktop/flutter/lib/main.dart:260-266` | Dans la barre latérale élargie (`180 px`), `Text('MicCamWatch')` dans `Row` n'était pas entouré d'`Expanded`, provoquant un débordement horizontal (`RenderFlex overflow`) aux grands facteurs de texte (1,5×–2,0×). | **Corrigé et testé** (`a256748`) |
| **F5** | **P2** | `desktop/flutter/lib/preferences.dart:429-437` | `pauseScope` (7 langues) indiquait encore une durée figée de « 30 minutes » alors que le bouton « Autre durée… » (1–10080 min) est disponible. De plus, `DropdownButton<String>` gardait une hauteur fixe (`48 px`) susceptible de rogner le texte à 2× et `_PreferenceLabels` pouvait lever une `Null check error` sur clé inconnue. | **Corrigé et testé** (`f50b477`) |
| **F6** | **P2** | `desktop/flutter/lib/main.dart:270,291,458`, `consultation_panels.dart:290,523`, `tools_panels.dart:167` | (a) `Tooltip` dupliquait l'annonce lecteur d'écran des boutons de navigation ; (b) le bouton de thème n'avait ni `ValueKey` ni `Semantics(identifier: 'theme.toggle')` et le `Wrap` d'en-tête n'était pas étendu en largeur ; (c) `About` affichait toujours `· development` même en mode `operational` ; (d) `LinearProgressIndicator` ignorait `disableAnimations` (réduction de mouvement) ; (e) les lignes localisées `notification_authorization` n'étaient pas sélectionnables ni trimmées sur `;`. | **Corrigé et testé** (`a256748`) |
| **F7** | **P2** | `desktop/flutter/windows/runner/win32_window.cpp` | Pas de gestionnaire `WM_DISPLAYCHANGE` pour recadrer une fenêtre déjà ouverte si la résolution diminue ou si un écran externe est retiré. Persistance géométrique Windows/Linux ouverte. | **Documenté (non modifié)** — pas de rebuild C++ Windows effectué |
| **F8** | **P0** | `native/macos_process.swift` (ouvert avant cette revue) | Terminaison d'une application utilisateur macOS durcie (cas Vorssaint, UID 501) : `task_for_pid` échoue en Mach 5 (`KERN_FAILURE`) sous SIP/Hardened Runtime. | **Ouvert — décision sécurité requise** (ne pas modifier sans revue dédiée) |
| **F9** | **P1** | `native/macos_desktop.swift` / `src/platform/macos.rs:535-560` | Bannières macOS non prouvées sur session réelle ; helper ad hoc extrait dans un répertoire temporaire différent à chaque processus. | **Ouvert — matériel/signature requis** |
| **F10** | **P3** | `src/watcher/unix.rs` (`deliver_alerts`, `c4d3b3b`) | `platform::play_chime()` n'est pas gardé par `!stopped.load(Ordering::Acquire)` après `Settings::load()`, contrairement à `notify_access`. | **Documenté (non modifié)** — à vérifier/corriger côté Unix |

---

## 6. Corrections apportées (lot non sensible)

1. **Commit `f50b477`** — `Explain applied Windows profile and variable pause durations`
   - `desktop/flutter/lib/preferences.dart` : `profileEffect` Windows suit désormais `view.appliedProfile` (et affiche `profileEffect_unknown` traduit dans les 7 langues si `appliedProfile == null`) ; mise à jour de `pauseScope` dans les 7 langues ; `itemHeight: null` sur `DropdownButton<String>` ; garde de repli sûr dans `_PreferenceLabels`.
   - `desktop/flutter/test/preferences_test.dart` : ajout du test `Windows profile description follows applied policy, not saved choice` couvrant `saved != applied` et `applied == null` dans les 7 langues.
2. **Commit `22ffd98`** — `Cache Flutter themes and suppress unchanged idle rebuilds`
   - `desktop/flutter/lib/main.dart` : mise en cache statique de `_lightTheme` et `_darkTheme` (plus de recalcul `ColorScheme.fromSeed` à chaque snapshot/tick) ; isolation du texte d'horloge locale dans `_LocalClockText` (1 seul `Text` mis à jour/s).
   - `desktop/flutter/lib/snapshot.dart` : `SnapshotSession.tick()` ne notifie les abonnés que lors d'une vraie transition d'état ou de `transportLive` (la détection de péremption à 1 s reste strictement identique).
   - `desktop/flutter/lib/{commands,preferences,consultation}.dart` : suppression des `notifyListeners()` redondants lorsque séquence, validité et connectivité n'ont pas changé.
   - `desktop/flutter/test/{snapshot,commands,preferences}_test.dart` : 3 nouveaux tests unitaires vérifiant la suppression des notifications redondantes **et** le maintien immédiat des notifications lors d'un passage `live -> stale`, d'une expiration `transportLive` en état déjà `stale`, ou d'un basculement de connectivité.
3. **Commit `a256748`** — `Fix Flutter shell overflow, Material panel ink/focus, accessibility and add shell tests`
   - `desktop/flutter/lib/main.dart` : remplacement du `Container` opaque de `_panel` par `Material(color: c.card, shape: RoundedRectangleBorder(...))` (rétablit le focus/ink de `SwitchListTile` et supprime l'assertion framework) ; `Expanded` sur `Text('MicCamWatch')` dans la barre latérale ; `excludeFromSemantics: true` sur les `Tooltip` de navigation ; `SizedBox(width: double.infinity)` sur le `Wrap` d'en-tête ; `ValueKey('theme.toggle')` et `Semantics(identifier: 'theme.toggle')` ; suppression du suffixe `· development` dans `About` lorsque `mode == 'operational'` ; paramètres optionnels de test sur `MicCamWatchApp` sans aucun changement de comportement en production.
   - `desktop/flutter/lib/consultation_panels.dart` et `tools_panels.dart` : `SelectableText` + `.trim()` pour le diagnostic `notification_authorization` ; respect de `MediaQuery.disableAnimationsOf(context)` sur `LinearProgressIndicator`.
   - `desktop/flutter/test/app_shell_test.dart` : nouvelle suite complète du shell `MicCamWatchApp` exerçant `640×440`, `680×440`, `1040×700`, échelles texte `1,5×` et `2,0×`, les 7 langues, les 2 thèmes, les 6 pages, la réduction de mouvement, la navigation clavier `Tab` + `Entrée`, le basculement de thème, l'affichage `About` opérationnel, l'émission `ready` et la déconnexion propre du flux.

---

## 7. Tests exécutés et nature des vérifications

| Suite / Commande | Type d'environnement | Résultat | Détails |
| --- | --- | --- | --- |
| `flutter test --no-pub` (sur `16d3012` avant corrections) | Tests widget/unitaires Dart locaux (Windows x64, Flutter local SDK 3.14 Dart) | **PASS : 120 / 120** | Référence initiale reproduite |
| `flutter test --no-pub` (après les 3 commits, sur `a256748`) | Tests widget/unitaires Dart locaux | **PASS : 126 / 126** | +6 nouveaux tests, 0 échec, 0 exception de layout |
| `flutter analyze --no-pub` (sur `a256748`) | Analyse statique Dart locale | **0 erreur, 0 warning** | Uniquement 16 `info` de dépréciation `package:flutter/material.dart` dus au SDK local plus récent que Flutter 3.47.8 épinglé en CI (15 préexistants + 1 pour `app_shell_test.dart`) |
| `cargo test --lib --locked --features flutter-desktop history::reader::tests` | Tests unitaires Rust locaux (sandbox Windows x64) | **PASS : 11 / 11** | Inclut `reverse_scan_preserves_partial_lines_across_byte_budget_and_io_blocks` |
| `cargo test ... frontends::tui::tests::event_log_rebaselines` | Tests unitaires Rust locaux (sandbox) | **PASS : 1 / 1** | Rebasage TUI Windows après trou de collecte |
| `cargo test ... output::tests::status_json_preserves_gaps` | Tests unitaires Rust locaux (sandbox) | **PASS : 1 / 1** | Sérialisation JSON `observation_gaps` |
| `cargo test ... frontends::desktop::preferences::tests` | Tests unitaires Rust locaux (sandbox) | **PASS : 21 / 21** | Contrats préférences/profil/pause |
| `cargo test ... --features flutter-desktop,packaged-desktop updater::bundle:: updater::recovery::` | Tests unitaires maintenance Rust (sandbox) | **PASS : 5 / 5** (1 ignoré : ZIP release requis) | Contrôle bundle + logique récupération MSI |
| `cargo test ... updater::` (autres tests maintenance) | Tests unitaires Rust (sandbox Windows x64) | **23 PASS, 3 ignorés, 6 bloqués sandbox (`os error 5`)** | Les 6 échecs (`namespace_uses_canonical_identity_and_refuses_a_different_data_root`, `a_restart_marker_blocks_verification_during_the_same_boot`, et les 4 `updater::portable::tests`) échouent sur `Accès refusé (os error 5)` car le bac à sable bloque l'accès à `%LOCALAPPDATA%\MicCamWatch` (préservant les données réelles v0.21.0). Aucune régression produit constatée. |

---

## 8. Mesures CPU/RAM

- **Aucune nouvelle mesure système privilégiée ou runner CI n'a été déclenchée** (conformément au budget et au périmètre).
- **Mesures existantes auditées (`docs/HYBRID_STAGE4_AUDIT.md`, `16d3012`) :**
  - **Linux x64 Xvfb (`199436c`, rejeu sans rebuild run `45436952897`) :** Impeller/llvmpipe = 272,81–273,17 % CPU, 284,45–298,30 Mio RSS ; Skia logiciel explicite = 1,40–1,60 % CPU, 134,42–136,24 Mio RSS ; après fermeture = 1 processus, 9,88–10,03 Mio RSS, 0,27–0,33 % CPU. Limite : Xvfb court sans GPU/Wayland ni capture active.
  - **Windows x64 MSI installés (`7252421`, runner admin run `45435090256`) :** v0.21.0 tray seul = 1,75 % CPU / 16,66 Mio RSS ; v0.22.0 tray après fermeture = 0,52 % CPU / 18,44 Mio RSS ; v0.21.0 fenêtre native = 2,07 % CPU / 25,18 Mio RSS ; v0.22.0 fenêtre Flutter = 0,93 % CPU / 96,14 Mio RSS (63,82 Mio commit privé, 2 processus, 34–37 threads). Limite : 15 s au repos, points de départ d'ouverture non comparables (298 ms vs 1 105 ms), pas de capture active ni AB/BA sur 180 s.
- **Impact attendu du commit `22ffd98` :** élimine 1 reconstruction complète de `MaterialApp` + `ColorScheme.fromSeed` + 3 notifications clients par seconde au repos, ainsi que le recalcul de `ThemeData` sur chaque frame snapshot, sans changer aucune cadence de collecte ni fenêtre de fraîcheur. L'effet CPU/RAM réel devra être mesuré lors de la prochaine campagne de build/rejeu CI (Windows et Linux).

---

## 9. Limites des qualifications

Ce qui **n'a pas pu être exécuté ni prouvé** dans cette session (distinction stricte code/tests unitaires vs matériel/VM/CI) :
- **macOS (matériel physique et runners) :** aucune compilation Swift/Rust macOS ni test natif `macos_process.swift` / `macos_desktop.swift` / CoreMediaIO exécuté localement (poste Windows x64). Cas Vorssaint, reprise caméra après veille et bannières réelles avec permission persistante restent **ouverts**.
- **Linux (x64 accéléré, Wayland, ARM64 GUI) :** aucune mesure GPU/Wayland ni exécution graphique ARM64 réalisée ici ; la revue du runner Linux (`my_application.cc`) et des scripts (`linux.py`) est basée sur le code et les reçus CI existants.
- **Windows VM `MCW-Win11-x64-Lab` et MSI compte standard :** aucune transaction MSI ni accès Hyper-V effectués ; la qualification MSI en compte standard sur révision post-`e74a86d` reste **explicitement ouverte**.
- **Performances durables / capture active :** aucune mesure AB/BA de 180 s avec micro/caméra actifs, changement de thème/langue en continu ou reprise après veille.
- **Accessibilité complète :** les tests widget vérifient la traversée clavier (`Tab`/`Entrée`), les identifiants sémantiques, le zoom 200 % et la réduction de mouvement ; un parcours complet avec lecteur d'écran réel (NVDA/Narrator, Orca/AT-SPI, VoiceOver) reste à qualifier sur les binaires compilés.

---

## 10. Décisions de sécurité restant à prendre

1. **Terminaison macOS (Vorssaint, P0) :** valider avec revue indépendante si MicCamWatch doit :
   - (a) ajouter une action distincte et explicite **« Demander la fermeture normale »** reposant sur `NSRunningApplication.terminate()` (uniquement pour les applications GUI enregistrées dont l'identité d'instance est corrélée à `libproc`, avec observation séparée de la sortie et sans la présenter comme une terminaison forcée), tout en conservant le refus honnête de `task_for_pid` sur cibles durcies ;
   - ou (b) documenter formellement la limite OS sur les apps durcies sans ajouter de second chemin d'action.
2. **Identité du helper de notifications macOS (P1) :** décider si le helper embarqué peut être logé à un chemin stable à l'intérieur du bundle signé `MicCamWatch.app` (pour éviter le chemin `/tmp/mcw-*` recréé à chaque lancement), sous revue de signature/scellement/Developer ID.
3. **Qualification MSI compte standard x64 (P1) :** planifier une passe autorisée dans `MCW-Win11-x64-Lab` sur le MSI de la révision consolidée avant toute proposition de RC (étape 5).
4. **Persistance de géométrie Windows/Linux et `WM_DISPLAYCHANGE` (P2) :** décider si ce point reste reporté ou traité avant la RC.

---

## 11. Commits locaux créés sur `work/stage4-neon-review`

| Ordre | SHA | Message |
| :---: | --- | --- |
| 1 | `f50b4778c0bd0b449a04d5d6eae533af4dec4c1e` | `[CF-Pages-Skip] [skip ci] Explain applied Windows profile and variable pause durations` |
| 2 | `22ffd9852752cff51259248a5b74470049bcf62e` | `[CF-Pages-Skip] [skip ci] Cache Flutter themes and suppress unchanged idle rebuilds` |
| 3 | `a256748da020ab3b26b9518f1eb91df7617941cb` | `[CF-Pages-Skip] [skip ci] Fix Flutter shell overflow, Material panel ink/focus, accessibility and add shell tests` |
| 4 | *(commit documentaire suivant)* | `[CF-Pages-Skip] [skip ci] Record independent stage 4 review and Neon handoff` |

---

## 12. Différences par rapport à `work/flutter-hybrid-0220` (`16d3012`)

Périmètre strictement limité à `desktop/flutter/lib/`, `desktop/flutter/test/` et `docs/HYBRID_STAGE4_NEON_HANDOFF.md` :
- **0 fichier Rust (`src/`) modifié.**
- **0 fichier natif Swift/C++ (`native/`, `desktop/flutter/{windows,linux,macos}/`) modifié.**
- **0 fichier MSI, installateur, workflow CI ou site web modifié.**
- Diff code + tests Flutter (`16d3012..a256748`) : 11 fichiers, `+493 / -52` lignes (dont 205 lignes dans le nouveau `desktop/flutter/test/app_shell_test.dart`).

---

## 13. Étapes de reprise proposées pour Codex (GPT-6.2 Sol)

1. **Relire et cherry-picker (ou fusionner après revue)** les 3 commits de code (`f50b477`, `22ffd98`, `a256748`) + ce rapport depuis `work/stage4-neon-review` vers `work/flutter-hybrid-0220` si validés.
2. **Valider en CI sur Flutter 3.47.8 épinglé** (`flutter analyze` + `flutter test` + scénarios `UiQualification`) pour confirmer 0 avertissement sur le SDK CI exact et valider l'absence de régression dans les scénarios instrumentés (`consultation`, `parity`, `filters`, `maintenance`, `quit`).
3. **Traiter le petit point P3 Unix** dans `src/watcher/unix.rs` (`deliver_alerts` : garder `play_chime()` avec `!stopped.load(Ordering::Acquire)`).
4. **Trancher les décisions de sécurité P0/P1 macOS** (§10 : voie `NSRunningApplication.terminate()` explicite vs documentation de limite pour Vorssaint ; chemin stable du helper de notifications dans `.app` vs temp dir).
5. **Rejouer la qualification MSI compte standard** dans `MCW-Win11-x64-Lab` sur le MSI consolidé et compléter les campagnes de performance comparatives (capture active, reprise, Linux accéléré) avant d'évaluer la clôture de l'étape 4.
