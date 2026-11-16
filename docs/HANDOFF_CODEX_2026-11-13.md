# Passation Codex — 13 novembre 2026

**Décision de l'utilisateur : arrêter ici le travail d'implémentation mené avec
oh-my-pi, ne rien supprimer, tout consigner pour reprendre dans Codex.**
Ce document ne remplace aucune preuve ni aucun document existant ; il les
relie. Aucune ligne de code produit n'a été modifiée après `f70c7f7`.

**Verdict conservé : ÉTAPE 2/6 INCOMPLÈTE.** Les familles **O02** (session et
cadence avancées) et **O05** (sessions et sorties explicites) restent ouvertes.
Le vert de la CI, les builds et les captures ne les terminent pas.

---

## 0. Résumé en une page

| Élément | Valeur vérifiée |
|---|---|
| Dépôt de travail | `C:\Users\stcui\Documents\mcw-flutter-0220` |
| Branche | `work/flutter-hybrid-0220` |
| HEAD avant ce document | `f70c7f720976299a6ce956bdb732261dacf381dc` |
| Dépôt distant | identique au HEAD local (aucun écart) |
| Arbre de travail | propre avant ce document |
| Base de départ de la mission | `93e8003` |
| Commits ajoutés | **17** (14 fonctionnels/outillage + 3 documentaires, puis les commits de passation) |
| Différentiel | 41 fichiers, **+3214 / −134** lignes |
| Source produit qualifiée | `6094f3a15c980d465ea70abd16442f7c59567705` |
| Correctif de garde de test | `5ed7303e8ebdd610c7ae7ece9f16800832138e6a` (script de test seulement) |
| Run quatre cibles Unix | [43402431543](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/43402431543) — **succès** |
| Run gardes natives | [43406668221](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/43406668221) — **succès** |
| `main` préservée | `73369abda09bdadce8c779a9559b3e2334a32ecb` |
| Tag stable préservé | objet `06d800ba536b5e6945abe4a4345957ba85c79cbf` → commit `3d74686ace7b04728794765f9eb57c0c0a8bc84c` |
| Worktrees préservés | 12 |
| Stash préservé | `mcw-local-handoff-preserved-before-0.17.2-sync-20261013` |
| Suppressions historiques | 215, non restaurées, non annulées |
| Publication | **aucune** (pas de release, tag, fusion main, déploiement) |
| Installation quotidienne | inchangée ; cinq fichiers témoins identiques |
| Sous-agents | **zéro** (routage modèle/fournisseur/authentification non prouvé) |

**Livré et vérifié :** caméra Windows déléguée au propriétaire natif ;
enforcement piloté par une session Rust ; Quitter MicCamWatch distinct de
fermer Flutter ; consultation paginée de tous les accès d'une instance ;
maintenance/version sans transaction hybride non qualifiée ; filtres de
présentation réels ; protocole IPC 9 ; scopes traduits en sept langues ;
campagnes de qualification Windows, Linux x64/ARM64 et macOS Intel/Apple Silicon.

**Reste :** O02 et O05 uniquement, avec les deux décisions de portée prises par
l'utilisateur le 13 novembre 2026 (§4). Leur implémentation n'a pas commencé.

---

## 1. État Git exact et préservation

Vérifications exécutées avant la rédaction de ce document :

```console
git rev-parse --abbrev-ref HEAD      -> work/flutter-hybrid-0220
git rev-parse HEAD                   -> f70c7f720976299a6ce956bdb732261dacf381dc
git status --porcelain=v1            -> (vide)
git ls-remote origin refs/heads/work/flutter-hybrid-0220
                                     -> f70c7f7… (identique)
git rev-parse main                   -> 73369abda09bdadce8c779a9559b3e2334a32ecb
git rev-parse v0.21.0                -> 06d800ba536b5e6945abe4a4345957ba85c79cbf (annoté)
git rev-parse 'v0.21.0^{}'           -> 3d74686ace7b04728794765f9eb57c0c0a8bc84c
git worktree list                    -> 12 entrées
git stash list                       -> 1 entrée (ci-dessus)
```

- Aucune branche protégée n'a été réécrite, forcée ou fusionnée.
- Les modifications historiques non commitées de `work/github-visibility`
  (215 suppressions) n'ont pas été restaurées ni annulées.
- Les preuves de l'installation quotidienne (`stable-before.json`,
  `stable-after.json`) montrent **cinq fichiers témoins strictement identiques** :
  deux EXE MSI, les préférences, l'intention microphone et l'état caméra.
- Aucun service, pilote, périphérique ou compte utilisateur n'a été modifié.

---

## 2. Ce qui a été fait — commit par commit

Les commits sont dans l'ordre chronologique. « Docs » signifie documentation et
preuves seulement, sans changement de comportement.

| Commit | Nature | Contenu |
|---|---|---|
| `e7e990e` | fonctionnel | Caméra : commandes déléguées au propriétaire natif, tampon de génération, capacités/confirmations, reprise et récupération legacy explicites ; nouveau `desktop/camera_owner.rs` |
| `ce8cbb2` | docs | Preuves et limites caméra |
| `d90fa7f` | fonctionnel | Enforcement piloté par la session Rust : `watcher/enforcement.rs`, bail de session, opt-in Flutter, refus en mode développement, arrêt qui attend l'effet admis |
| `432153c` | docs | Sémantique et preuves d'enforcement |
| `79f9de3` | fonctionnel | « Quitter MicCamWatch » via le propriétaire : `desktop/exit_owner.rs`, préparation à usage unique, révision native, arrêt coopératif, distinct de la fermeture de Flutter |
| `ee8daa8` | docs | Preuves de sortie contrôlée |
| `b2ac919` | fonctionnel | Multi-accès : `desktop/applications.rs`, instantané par PID, curseur privé à usage unique, budget JSON, troncature annoncée, pagination réelle |
| `baf302d` | docs | Preuves multi-accès et IPC 8 |
| `4e7b20b` | fonctionnel | Maintenance : `desktop/maintenance.rs`, propriétaire d'installation, recherche de version passive, refus des transactions hybrides incomplètes dans update/uninstall/worker |
| `116a0a0` | docs | Preuves de maintenance passive et refus |
| `85c6493` | fonctionnel | Filtres de présentation : `observation_view.dart`, ressource/risque/Ready, sans changer sondes, cadence ou protections ; protocole 9 ; six scopes traduits |
| `b6dccc5` | docs | Preuves de filtres réels |
| `6094f3a` | outillage | `macos_quit.py` (vraie sortie Flutter), garde locale du harness, cibles Unix ; tests d'enforcement et de sortie |
| `a384b9d` | docs | Réconciliation des 44 familles, écarts O02/O05 explicites |
| `5ed7303` | outillage | Garde Linux : refuse la sélection vide et exige les deux noms de tests avant exécution |
| `1f6f82b` | docs | Preuves Unix du run complet |
| `f70c7f7` | docs | Bilan de fin d'étape, statut honnête |

Détails utiles :

- **Caméra** : le chemin Flutter ne fait plus d'hypothèse sur une action
  matérielle ; il prépare, confirme, puis relit l'état natif. Les tests utilisent
  des doubles de propriétaire ; UAC et caméra physique ne sont pas exercés.
- **Enforcement** : un seul collecteur. L'autorité de session vit dans Rust ;
  fermer/réouvrir Flutter ne la perd pas, redémarrer Rust la désactive.
  L02 (sessions/journaux) n'est pas résolu par ce travail.
- **Quitter** : quatre arrêts réels demandés depuis Flutter ont réussi
  (clair/sombre, deux propriétaires neufs par thème), avec code de sortie 0 et
  aucune garde désactivée. Fermer la fenêtre reste un autre chemin.
- **Multi-accès** : un vrai test desktop a trouvé une collision de champ
  `enforcement` (décision d'accès contre statut d'exécution). Le runtime est
  désormais `enforcement_runtime` ; l'ancien protocole est refusé, sans shim.
- **Maintenance** : aucune transaction hybride n'est simulée en succès. Une
  consultation de version n'est ni un téléchargement ni une preuve de signature.
  Les fonctions natives sans `flutter-desktop` restent intactes.
- **Filtres** : locaux à la fenêtre, bornés, sans seconde session `watch` ;
  une vue filtrée n'est pas une preuve d'absence de capture.

---

## 3. Preuves réellement examinées

### 3.1 Campagne quatre cibles Unix

Run [43402431543](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/43402431543),
source `6094f3a`. Archives téléchargées et conservées hors dépôt ; SHA-256
comparés aux manifests ; `built-products/source-commit.txt` contrôlé pour les
deux Macs. Les JSON runtime et scellements ont été lus, pas seulement le statut
des jobs.

| Cible | Tests réellement réussis | Exécution graphique / limite |
|---|---|---|
| Linux x64 Ubuntu 22.04 | 79 desktop + 3 enforcement + 5 settings + 12 history + 9 autostart ; 115 Flutter ; Clippy/analyse/release | 6 cycles lifecycle + 6 cycles préférences, deux thèmes, SNI réel, écritures/relectures privées, propriétaire à 0, aucun frontend résiduel ; Xvfb logiciel |
| Linux ARM64 Ubuntu 24.04 | Mêmes 79/3/5/12/9 et 115 Flutter ; Clippy/analyse/release ; ELF aarch64 | **Pas d'exécution de l'application**, GPU, SBC/ARM physique ni contrôle matériel |
| macOS Intel | 78 desktop + 3 enforcement + 5 settings + 12 history + 7 autostart + 2 gardes ; 115 Flutter ; Clippy/analyse/release | 8 scénarios langue/thème, 24 close/reopen, 32 preuves de panneaux, deux Quit réels, propriétaire à 0, NSWindow visible, préfixe sans processus |
| macOS Apple Silicon | Mêmes 78/3/5/12/7/2 et 115 Flutter | Mêmes 24 cycles/32 preuves et deux Quit réels ; aucun test manuel |

Sur chaque Mac : runtime normal non instrumenté avec IPC réel, sandbox
`com.apple.security.app-sandbox` inchangée, terminaison des enfants après la
mort du seul parent privé, cycle de fenêtre/menu dans le build instrumenté
distinct. Quatre captures ScreenCaptureKit des seules fenêtres possédées,
examinées en clair/sombre sur les deux architectures. Les fixtures AppKit
simulées restent distinctes des captures Flutter réelles.

### 3.2 Gardes natives ciblées

Run [43406668221](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/43406668221),
source `5ed7303`. Les quatre artefacts ont été téléchargés et vérifiés :
source exacte dans chaque artefact, **exactement les deux noms attendus**
sélectionnés, puis **2 passed / 0 failed** sur Linux x64, Linux ARM64,
macOS Intel et macOS Apple Silicon. Aucun rebuild Flutter.

Cela corrige un défaut de **test**, pas du produit : l'ancien sélecteur Linux
utilisait un namespace inexistant et Cargo renvoyait « 0 passed » sans échec.
Le zéro test initial est conservé dans la documentation ; il n'est jamais
présenté comme un succès de garde.

### 3.3 Où sont les preuves

Répertoire durable :
`%LOCALAPPDATA%\MicCamWatchQualification\v022-stage2-final-20261112-084140`

Fichiers de synthèse :

- `final-handoff.json` — état Git, runs, décisions, limites ;
- `native-gates-verified.json` — quatre artefacts de gardes vérifiés ;
- `unix-verified-receipts.json` (~487 Ko) — reçus Unix détaillés ;
- `stable-before.json` / `stable-after.json` — cinq fichiers témoins ;
- `native-gates-result.json`, `native-gates-artifacts/` ;
- `unix-ci-*`, `macos-*-receipts`, `macos-*-real.png`, `view-v9-*`,
  `applications-v8-*`, `maintenance-*`, `explicit-quit-v2-*` ;
- scripts de qualification et leurs premiers échecs.

Ce répertoire est **hors dépôt** et doit être conservé ; GitHub ne le rétentionne
pas à la place de l'utilisateur.

### 3.4 Ce que ces preuves ne disent pas

Le vert CI ne prouve pas : matériel physique, TCC/UAC/Polkit réels, GPU,
Mint/Ubuntu accéléré, ARM/SBC physique, Developer ID/notarisation, paquet
hybride installé, transaction update/uninstall complète, Windows 10/DPI/
accessibilité certifiés, ni absence de capture. Ces limites sont détaillées au §8.

---

## 4. Décisions de portée validées par l'utilisateur (13 novembre 2026)

Deux choix ont été soumis et acceptés. Ils **bornent** O02/O05 ; ils ne les
implémentent pas et ne les ferment pas.

### 4.1 O02 — `presentation_only`

> « Conserver la cadence de surveillance. Flutter règle seulement la diffusion
> des résultats et affiche explicitement la cadence réelle disponible. Accepter
> cette différence avec le `--interval` de la CLI pour v0.22.0. »

Conséquences obligatoires :

- **Aucun** changement de la cadence de collecte native, aucun `--interval`
  exposé à Flutter, aucun second collecteur.
- La cadence demandée par l'utilisateur est une **cadence de présentation** ;
  elle ne peut pas être plus rapide que la cadence native observée et ne doit
  jamais la ralentir silencieusement.
- L'UI affiche deux valeurs distinctes : cadence native effective et cadence de
  présentation demandée/effective, avec la source de chaque valeur.
- La parité `watch --interval` n'est **pas** revendiquée pour v0.22.0 ; c'est une
  différence de contrat assumée, documentée.
- Les tests doivent prouver que le nombre d'appels au collecteur ne change pas
  quand la cadence de présentation change, et qu'aucun START/STOP n'est inventé.

Cadences natives à constater, pas à supposer :

- CLI `mcw watch` : `--interval` 750 ms par défaut, minimum 100 ms
  (`src/frontends/cli.rs::WatchOptions`) ; c'est une vraie cadence de collecte.
- UI Flutter Unix : `INTERVAL = 2 s` dans `src/frontends/tray/unix.rs` pour la
  lecture d'observation.
- UI Flutter Windows : boucle d'observation `park_timeout(500 ms)` dans
  `src/frontends/tray/windows_observer.rs` ; inventaire caméra tous les 5 s
  (`CAMERA_POLL_INTERVAL` dans `src/frontends/tray.rs`).
- Ces valeurs doivent être exposées par le propriétaire, pas recopiées en dur
  dans Flutter.

### 4.2 O05 — `owned_directory`

> « Fichiers JSONL nouveaux dans un répertoire de sessions possédé par Rust,
> avec chemin affiché et confirmation de l'enregistrement de données locales.
> Aucun chemin arbitraire fourni par Flutter. »

Conséquences obligatoires :

- L'option `--log` de la CLI garde son contrat propre ; Flutter ne peut
  **jamais** fournir un chemin.
- Le répertoire est résolu et créé par Rust, de préférence via l'aide existante
  `settings::data_dir()` puis `sessions` :
  - Windows : `%LOCALAPPDATA%\MicCamWatch\sessions\` ;
  - Unix : `$XDG_DATA_HOME/MicCamWatch/sessions` ou
    `~/.local/share/MicCamWatch/sessions` ;
  - macOS : même règle Unix (choix existant de `data_dir`, pas une nouvelle
    convention `Application Support`).
- Chaque démarrage crée un **nouveau** fichier, jamais un écrasement :
  nom généré par Rust (horodatage UTC + suffixe aléatoire), `.jsonl`.
- Droits et identité : répertoire privé, fichier 0600 sur Unix ; création
  exclusive (`create_new`), refus des liens symboliques/reparse points et des
  remplacements ; vérification que le parent est bien le répertoire possédé.
- Une confirmation explicite de l'utilisateur précède l'enregistrement de
  données locales ; le chemin exact est ensuite affiché.
- Erreurs disque, quota ou permission **visibles** dans l'UI et dans l'état de
  session ; jamais de perte silencieuse d'événements.
- Le journal de session est indépendant de `history_enabled` : le désactiver ne
  doit pas arrêter une session explicitement consentie, et l'historique ne doit
  pas être présenté comme ce journal.
- Fermer Flutter ne ferme pas la session possédée par Rust ; une sortie du
  propriétaire la termine proprement (flush + pied de session).

---

## 5. Ce qui reste — O02 (session et cadence de présentation)

### 5.1 Contrat attendu

Une **session d'observation avancée possédée par le moteur unique** :

- état : `Idle` / `Recording` / `Stopping` / `Failed` ;
- champs de statut : `session_id`, `started_at`, `requested_presentation_ms`,
  `effective_presentation_ms`, `native_cadence_ms`, `origin` (native/UI),
  `events_written`, `bytes_written`, `path`, `last_error` ;
- démarrage par préparation + confirmation à usage unique (comme les commandes
  existantes), avec consentement explicite ;
- arrêt idempotent, avec relecture du fichier et du nombre d'événements ;
- survie à la fermeture/réouverture de Flutter ; arrêt à la sortie du
  propriétaire ;
- aucune baisse cachée de fréquence : si la cadence demandée est inférieure à la
  cadence native, le propriétaire la borne et l'affiche.

### 5.2 Carte du code à réutiliser

| Élément | Fichier / symbole |
|---|---|
| Snapshot complet côté propriétaire Unix | `src/frontends/tray/unix.rs`, `Reading { snapshot, … }` |
| Snapshot complet côté propriétaire Windows | `src/frontends/tray/windows_observer.rs`, boucle d'observation |
| Projection bornée vers Flutter | `src/frontends/desktop.rs::DesktopSnapshot::from_snapshot` |
| Publication et changements de présentation | `Frontend::refresh` / `Supervisor::take_presentation_changed` |
| Commandes typées et confirmations | `src/frontends/desktop/tools.rs::{Request, Report, Session}` |
| Transitions natives existantes | `src/watcher/unix.rs::{TransitionTracker, EventDispatcher}`, `src/watcher/windows.rs` |
| Modèle d'événement | `AccessEvent` (schéma 3), `src/model.rs` |
| Version IPC | `src/frontends/desktop.rs::PROTOCOL_VERSION` et `desktop/flutter/lib/snapshot.dart::protocolVersion` |

Point critique : **ne pas** alimenter un nouveau tracker avec les 128 lignes
projetées. Le propriétaire doit traiter le snapshot complet **avant**
`from_snapshot`, au même endroit que l'enforcement/les alertes, pour préserver
les trous d'observation et éviter les doublons.

### 5.3 Étapes proposées

1. **Cadence exposée** : ajouter au `DesktopSnapshot` un bloc borné
   `cadence { native_ms, presentation_ms, requested_ms }`, avec une seule source
   de vérité côté Rust. Choisir explicitement de bumper `PROTOCOL_VERSION` 9 → 10
   et mettre à jour `snapshot.dart`, `main.dart` (« Protocol … »), les fixtures et
   les tests de refus d'ancien protocole. Ne pas ajouter de champ non versionné.
2. **Session Rust** : nouveau module `src/frontends/desktop/session.rs`
   (état + `Recorder` + chemins possédés + erreurs typées). Il vit dans l'état du
   propriétaire (`tray.rs` / `tray/unix.rs` / `windows_observer.rs`), pas dans le
   `tools::Session` du frontend, sinon fermer Flutter tuerait la session.
3. **Branchement des transitions** : instancier un `TransitionTracker` dans le
   propriétaire, alimenté par chaque snapshot complet, et n'écrire que les
   transitions réelles. Les gaps de collecteur suspendent les transitions ; ne
   jamais fabriquer un STOP à la perte du collecteur.
4. **Publication** : ajouter une échéance de présentation
   (`next_presentation`) dans la boucle qui publie déjà ; `changed ||
   presentation_changed || now >= next_presentation`. La cadence de collecte
   reste strictement inchangée. Borner `presentation_ms` à la cadence native.
5. **IPC** : nouveaux `Request::{SessionStart, SessionStop, SessionStatus}` et
   `Report` correspondant, avec token de préparation, confirmation explicite,
   refus en mode développement, et validation stricte (pas de chemin).
6. **UI** : page « Session » (ou section de Contrôles) avec consentement,
   démarrage/arrêt, cadence demandée (par ex. 1 s / 2 s / 5 s), valeurs
   demandée/effective/native, chemin, compteur d'événements, erreurs, readback.
   Sept langues, deux thèmes, trois échelles de texte.
7. **Tests** : voir §5.4 ; puis mise à jour des 44 familles, de l'audit (§9.1),
   du plan et de la ROADMAP ; qualification Windows réelle + Unix/macOS.

### 5.4 Tests d'acceptation O02

- Le nombre d'appels `monitor.snapshot()` ne change pas quand la cadence de
  présentation change (compteur de test ou instrumentation de test).
- Une cadence demandée sous la cadence native est bornée et affichée comme telle.
- Aucun événement START/STOP dérivé d'une frame tronquée ou d'un gap.
- Fermer/réouvrir Flutter conserve la session et la cadence ; sortir du
  propriétaire l'arrête.
- Un redémarrage Rust ne réactive ni session ni enforcement implicitement.
- Refus : développement, confirmation absente/périmée, double démarrage,
  session déjà active, protocole ancien.
- UI : la valeur affichée correspond au readback Rust, pas à l'intention.

### 5.5 Pièges

- Ne pas confondre `observed_at` (fraîcheur) avec une cadence mesurée.
- Ne pas publier plus vite que le snapshot ne change : une boucle de publication
  plus rapide ne crée pas de nouvelles observations.
- Ne pas utiliser `Instant` sérialisé ; le contrat IPC reste en millisecondes
  entières et bornées.
- Ne pas réutiliser `--log` pour contourner le répertoire possédé.

---

## 6. Ce qui reste — O05 (journal de session possédé)

### 6.1 Contrat attendu

- Un fichier JSONL par session, créé par Rust dans le répertoire possédé.
- En-tête de session borné (version, id, date, consentement, cadences, version
  outil), puis des `AccessEvent` schéma 3, puis un pied de session (`stopped_at`,
  raison, compteurs). Le format exact doit être testé et documenté.
- Aucun chemin fourni par l'UI ; l'UI reçoit le chemin et le nombre d'événements.
- Erreurs visibles ; arrêt propre sur limite de taille ou disque plein.
- Le fichier n'est ni l'historique global ni un fichier de debug : pas de
  mélange, pas de suppression par `clear-history`.

### 6.2 Sécurité fichier

- Résolution : `settings::data_dir()?.join("sessions")`.
- Création du répertoire avec droits privés ; refus si un composant est un lien
  symbolique/reparse point.
- Fichier : `create_new(true)`, nom Rust, `.jsonl`, 0600 Unix.
- Refus des remplacements, liens durs inattendus et parents non possédés.
- Limite de taille explicite (par ex. 32 Mio) : la session s'arrête avec une
  erreur, elle ne tronque pas silencieusement.
- Flush après chaque ligne ; remontée d'erreur vers l'UI ; clôture sur arrêt du
  propriétaire.

### 6.3 Tests d'acceptation O05

- Deux sessions ne réutilisent jamais un fichier ; collision EEXIST retentée.
- Un lien symbolique (ou reparse point Windows) est refusé, pas suivi.
- Droits 0600 / parent privé vérifiés sur Unix.
- Limite de taille atteinte → erreur visible, session arrêtée, fichier lisible.
- `history_enabled=false` n'empêche pas une session consentie d'écrire ; activer
  l'historique ne crée pas de doublon dans le journal de session.
- Fermer Flutter n'interrompt pas l'écriture ; le propriétaire termine et flushe.
- Les événements correspondent aux transitions natives, sans doublon avec
  l'enforcement ou les notifications ; les gaps sont préservés.
- Redémarrage de l'UI : le statut retrouve le chemin et le compteur exacts.
- Refus si consentement absent, si chemin (même vide) est fourni, ou si le
  mode développement tente une écriture.

### 6.4 Pièges

- Ne pas écrire depuis le thread UI : le disque peut bloquer ; utiliser le worker
  existant ou un thread possédé avec backpressure bornée.
- Ne pas ouvrir le fichier avant la confirmation ; une préparation ne doit pas
  créer de fichier.
- Ne pas réutiliser `history::append` : verrou, rotation et chemin globaux ne
  sont pas le contrat de session.

---

## 7. Invariants à ne pas casser

1. **Un seul moniteur/collecteur.** Aucun second `mcw watch`, aucun service
   caché, aucun moteur Flutter.
2. **Pas d'événements synthétiques.** Ne jamais déduire START/STOP des lignes
   projetées, d'un snapshot tronqué ou d'une frame en retard.
3. **Fenêtres bornées.** `MAX_FRAME_BYTES`, `MAX_ACCESSES`, `MAX_ITEMS`,
   `MAX_STRING_BYTES` restent respectés ; la réduction est annoncée.
4. **Autorité native.** Les commandes passent par préparation + confirmation
   à usage unique ; l'UI ne reçoit jamais de droit matériel direct.
5. **Sortie possédée.** Fermer Flutter ne termine pas le moteur ; « Quitter
   MicCamWatch » passe par `exit_owner` et reste distinct.
6. **Aucun chemin arbitraire** depuis Flutter vers le système de fichiers.
7. **Sept langues** pour toute nouvelle chaîne visible.
8. **Pas de nouvelle permission silencieuse.** TCC/UAC/Polkit restent explicites.
9. **Ne pas toucher** `main`, le tag v0.21.0, l'installation stable, les 215
   suppressions historiques ni les worktrees.
10. **Ne pas publier** : aucune release, aucun déploiement, aucune fusion main
    sans instruction explicite.

---

## 8. Limites et non-preuves (à conserver telles quelles)

- **Matériel** : aucune capture micro/caméra physique n'a été provoquée. Les
  tests sensibles utilisent des doubles ; les périphériques quotidiens n'ont pas
  été modifiés.
- **Autorisations OS** : UAC, TCC et Polkit n'ont pas été exercés sur du matériel
  réel ; les chemins d'élévation restent non qualifiés.
- **Linux** : Xvfb logiciel, pas de GPU/Mint accéléré ; ARM64 = build/ELF/test
  hébergé, pas de SBC/ARM physique ni de périphérique.
- **macOS** : runners hébergés uniquement, pas de Mac manuel ; pas de Developer
  ID, notarisation, Gatekeeper ni TCC réels. TCC/capture/distribution restent
  expérimentaux.
- **Windows** : pas de Windows 10, DPI, accessibilité ou machine vierge
  certifiés ; pas de paquet MSI/ZIP hybride installé ni de transaction
  update/uninstall hybride qualifiée.
- **Produit** : une observation `Active` ou une décision de risque ne prouve pas
  qu'un périphérique est bloqué ; une liste vide ne prouve pas l'absence de
  capture.
- **CI** : un job vert ne remplace pas les scénarios matériels, et un artefact
  n'est pas une preuve d'installation.

---

## 9. Reprise pas à pas dans Codex

Depuis `C:\Users\stcui\Documents\mcw-flutter-0220` :

```console
git fetch origin
git switch work/flutter-hybrid-0220
git pull --ff-only
git status --porcelain=v1
git log --oneline --decorate -20
```

Lire dans cet ordre :

1. ce document ;
2. `docs/V0.22.0_PARITY_AUDIT.md` §9.1 et §9.2 ;
3. `docs/FLUTTER_MIGRATION_MATRIX.md` (tableau des 44 familles) ;
4. `docs/RELEASE_0.22.0_PLAN.md` §10 ;
5. `docs/ROADMAP.md` entrées du 13 novembre 2026.

Vérification ciblée avant toute modification :

```console
cargo fmt --check
cargo test --locked --features flutter-desktop
cargo clippy --locked --features flutter-desktop --all-targets -- -D warnings
cd desktop/flutter
flutter pub get --enforce-lockfile
flutter analyze --no-pub
flutter test --no-pub
```

Ne pas relancer une campagne CI coûteuse sans changement de code : les preuves
du 13 novembre couvrent `6094f3a` et `5ed7303`. Après une modification produit,
relancer la qualification ciblée :

```console
# quatre cibles Unix (Windows ignoré)
gh workflow run ci.yml --repo Roman-Cuisset/miccamwatch \
  --ref work/flutter-hybrid-0220 \
  -f native-only=true -f native-platform=flutter

# seulement les gardes natives Unix, sans rebuild Flutter
gh workflow run ci.yml --repo Roman-Cuisset/miccamwatch \
  --ref work/flutter-hybrid-0220 \
  -f native-only=true -f native-platform=flutter \
  -f flutter-native-gates-only=true
```

Ordre de travail recommandé :

1. O02 d'abord (cadence exposée + session Rust + UI + tests) ;
2. O05 ensuite (recorder possédé + UI + tests) ;
3. mise à jour des documents et des 44 familles ;
4. qualification Windows réelle, puis Unix/macOS ;
5. seulement ensuite discuter de l'étape 3 et du packaging.

---

## 10. Journal des incidents et corrections

Conservé pour éviter de refaire les mêmes essais :

1. **Sélecteur de garde Linux vide** (`x64` utilisait
   `frontends::tray::unix::menu_tests`, namespace inexistant) : Cargo renvoyait
   « 0 passed » sans échec. Corrigé dans `5ed7303` ; le run ciblé a ensuite
   exécuté exactement deux gardes sur chaque cible.
2. **`bash -n` en échec sur le worktree Windows** : fins de ligne CRLF ;
   le blob Git indexé est en LF et identique à l'entrée CI. Aucun changement de
   code produit ; les deux reçus sont conservés.
3. **Enforcement — fixture** : `Snapshot::clone()` indisponible ; corrigé
   uniquement dans la fixture, sans ajouter de copie au modèle produit.
4. **Enforcement — preuve visuelle** : le panneau se trouvait sous le viewport
   capturé ; documenté comme non prouvé visuellement à ce point, puis le
   harness a utilisé `Scrollable.ensureVisible` dans l'arbre possédé.
5. **Sortie — premier essai GUI** : le bouton avait une `ValueKey` mais aucun
   identifiant de sémantique pour le pilote ; ajout de l'identifiant accessible
   `tools.confirm`, puis quatre arrêts réels réussis.
6. **Applications — collision de champ** `enforcement` : décision d'accès et
   statut d'exécution confondus ; renommé `enforcement_runtime`, protocole 8,
   test de régression.
7. **Applications — suite Flutter** : 108 réussites et un échec de consommateur
   de la nouvelle clé ; panneau corrigé, 20/20 tests tools réussis. L'échec
   initial et ses reçus sont conservés.
8. **Build Flutter interrompu** : reçu « running » malgré une sortie de
   compilation ; seule l'étape interrompue a été reprise, sans relancer les
   tests déjà aboutis.
9. **Maintenance — code retour** : le premier harness attendait 1 alors que le
   produit renvoie 2 pour un refus d'update/uninstall hybride ; convention du
   produit inchangée, harness corrigé, échec conservé.
10. **Maintenance — build** : accès `pub(crate)` invalide depuis la crate
    binaire ; corrigé via l'entrée publique update existante, sans élargir l'API.
11. **Maintenance — preuve visuelle** : un premier déplacement ne montrait que
    le haut des contrôles ; non revendiqué comme preuve du panneau, remplacé par
    un défilement dans l'arbre possédé.

---

## 11. Fichiers de référence

- `docs/HANDOFF_CODEX_2026-11-13.md` — ce document.
- `docs/V0.22.0_PARITY_AUDIT.md` — §9, 44 familles, §9.1, §9.2.
- `docs/FLUTTER_MIGRATION_MATRIX.md` — inventaire de migration et clôture.
- `docs/RELEASE_0.22.0_PLAN.md` — plan en six étapes et état §10.
- `docs/ROADMAP.md` — journal chronologique et décisions.
- `docs/ARCHITECTURE.md` — invariants de frontend et contrats natifs.
- `docs/NEXT_AI_LINUX_MACOS.md` — reprise Linux/macOS et limites matérielles.
- `desktop/qualification/` — harness Linux/macOS.
- `%LOCALAPPDATA%\MicCamWatchQualification\v022-stage2-final-20261112-084140` —
  preuves durables (hors dépôt).

---

## 12. Ce qui n'a pas été fait, volontairement

- Aucune ligne de code produit n'a été modifiée après `f70c7f7`.
- Aucune suppression, réécriture d'historique, fusion `main`, release, tag,
  déploiement ou installation.
- Aucun sous-agent n'a été lancé.
- Aucun test matériel, UAC, TCC, Polkit, GPU ou ARM physique.
- O02/O05 ne sont **pas** implémentés ; les décisions du §4 ne sont pas encore
  dans le code. Ce document les rend exécutables, il ne les revendique pas.

**Prochaine action attendue dans Codex :** implémenter O02 avec la cadence de
présentation bornée, puis O05 avec le journal possédé, sans second collecteur ni
chemin arbitraire, et mettre à jour les 44 familles avec preuves réelles.
