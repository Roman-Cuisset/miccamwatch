# Roadmap technique MicCamWatch

Dernière mise à jour : **2026-10-13**. Responsable : lead architecture/développement.
Base publiée : [v0.17.2 stable/latest](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.17.2), source `a58c8b9f3a967589a576879d6d27bfdbc40f9dff` ; [CI exacte avant tag](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38361454701) et [release native](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38364052525) vertes ; sept assets/digests et six attestations exactes vérifiés. Paire PC publique 0.17.2 et vrai tray vérifiés après accord « Maintenant » ; aucune mutation de protection.

Ce document planifie les prochains travaux et consigne les lots livrés. Les fonctionnalités encore proposées ne sont pas annoncées comme disponibles ; les livraisons sont explicitement marquées dans le tableau et le journal. Les complexités sont des estimations de conception, pas des délais ni des mesures.

**Livré `v0.17.0` :** Windows-first explicitement retenu pour la protection
persistante. Les corrections ne réécrivent ni les défauts historiques ni les
preuves des anciennes releases. Les pins d'installation passent à `v0.17.0` ;
P1-RAM/P1-RESTORE sont **Publiés avec validation logiciel/natif**, sans
certification physique ni objectif 15 Mo acquis. Les autres lots demeurent
ouverts. Voir le [contrat de protection et ses limites](ARCHITECTURE.md#candidate-0170-windows-first-protection-contract).

## Base réelle et décisions d'architecture

- Déjà livré : collecteurs natifs, TUI partagée, tray Windows/StatusNotifier Linux/menu AppKit macOS, sons système optionnels, autostart explicite et mises à niveau gérées. La parité des frontends ne signifie pas une parité des permissions, de l'attribution ou du blocage matériel. Voir [Architecture](ARCHITECTURE.md).
- `mcw status --json` fournit un snapshot ; `mcw watch --json` fournit déjà du JSON par ligne, schéma **3**, événements START/UPDATE/STOP. Sous Unix, le flux contient aussi des documents de santé/récupération. Ne pas reconstruire ce moteur pour une barre d'état.
- Les règles TOML vivent dans **`policy.toml`** (`src/config.rs`) ; les préférences dans **`settings.toml`** (`src/settings.rs`). Un fichier nommé `config.toml` est possible via `--config`, pas comme troisième configuration automatique.
- Linux : PipeWire de la session et caméras USB/UVC compatibles avec helper séparé et Polkit. macOS : INPUT writable et caméra CoreMediaIO **sans PID client**, profil Restrictions à approuver manuellement. Windows : WASAPI et contrôle caméra soumis à UAC. Aucun de ces contrôles n'est un interrupteur électrique universel.
- Distribution actuelle : Windows x64 ZIP/MSI, Linux x64 glibc 2.35+, macOS 15+ ARM/Intel. **Pas de Linux ARM64 publié.** Windows est non signé sans certificat réel ; macOS n'a pas de Developer ID/notarisation. Les checksums/attestations ne sont ni une identité Authenticode ni un verdict antivirus ; voir [Signing](SIGNING.md).
- Linux 0.15.1 : son updater embarque l'ancien installateur à trois fichiers et refuse l'archive Linux 0.16.0 à six fichiers. La migration validée passe par l'installateur public courant au même préfixe ; ne pas réécrire une ancienne release pour réparer son client.

### Règles anti-bloat et sécurité communes

1. Garder le cœur Rust et les bridges natifs existants. Aucun runtime supplémentaire, framework web, serveur HTTP, lecteur multimédia, base embarquée ou SDK de messagerie pour les projets de cette roadmap.
2. **Cible souple : 15 Mo de RAM (15 000 000 octets) ; environ 30 Mo comme référence, pas seuil dur.** Ce choix remplace le seuil bloquant initial : ne pas déformer les mesures pour afficher moins de 15 Mo. Mesurer résident stable et pics, CLI/watch/top/tray séparément, avec tous les helpers/gardes MCW et enfants `pw-dump`/son. Publier la méthode de RSS partagée (une somme peut compter plusieurs fois les mêmes pages) et les lacunes d'échantillonnage/les enfants trop brefs. Aucun résultat « léger » en cachant le helper AppKit ou les pics de démarrage. Les scripts utilisateur externes ont un budget séparé : leur mémoire arbitraire ne peut pas être garantie par le cœur. Bornes explicites et absence d'accumulation restent obligatoires.
3. Qualifier CPU au repos, démarrage jusqu'au premier résultat, cadence et latence des événements sur chaque OS. Le watch actuel utilise notamment une cadence par défaut de 750 ms : « instantané » ne signifie pas détection physique garantie en temps réel.
4. Files, caches, réponses OS et sorties enfant bornés ; workers réutilisés ; pas de thread/processus par événement, de copie complète des preuves dans chaque file ou de travail coûteux lorsque l'option est désactivée.
5. Aucune collecte audio/vidéo pour mieux détecter. Aucune élévation implicite, modification TCC/SIP/PAM, exclusion Defender ou restauration de quarantaine. `Unknown`, attribution absente et résultats partiels restent visibles.
6. Une mutation et sa restauration doivent porter sur la même identité/génération et sur un état possédé. Ne jamais restaurer « tous les micros ouverts » ni déduire une autorité d'un nom de processus, d'un PID numérique seul ou d'une IP.

## Tableau de bord vivant

Les identifiants sont stables. À chaque livraison, remplacer l'état par **En cours**, **Validé**, **Publié** ou **Bloqué**, puis ajouter commit, release, preuves et reste à faire dans le journal final. « Publié » nécessite des artefacts accessibles et un smoke du chemin utilisateur ; une compilation seule ne suffit pas.

| Lot | Décision / état actuel | Complexité | Dépendance principale |
| --- | --- | --- | --- |
| P0-CAM-TRAY | **Publié 0.17.2 et installé sur PC**. Couverture partielle distincte, captures conservées, raisons natives détaillées ; Zoom PID brut valide/report FILETIME zéro, attribution prudente maintenue. Vrai tray PC avec Ktalk actif : menu compact, détails complets, icône rouge ; infobulle résiduelle suivie dans TRAY-WIN | Moyenne | Pas de certification matérielle ; timestamp nul reste Unknown, jamais autorité de kill |
| SITE-PAGES | **Livraison statique 0.17.2 historique ; incident placeholders signalé, correction urgente ouverte**. Export/ZIP et smokes précédents conservés comme preuves datées, pas comme réfutation du défaut actuel ; voir SITE-TOKENS et SITE-L10N | Moyenne | Corriger livraison réellement consultée ; projet Pages autorisé et HTTPS production après upload |
| SITE-TOKENS | **Urgent, demandé : à corriger avant nouvelle livraison web**. L'utilisateur voit `__MCW_VERSION__`, `__MCW_SOURCE__`, `__MCW_INSTALLER_SHA256__` à la place des valeurs ; identifier et réparer le chemin source/build/archive/publication concerné | Faible à Moyenne | Valeurs de release publiées uniques, aucun token visible ou URL/commande/checksum invalide |
| SITE-L10N | **Urgent, demandé : site complet dans les sept langues MCW** : anglais, français, allemand, espagnol, japonais, chinois simplifié, russe ; pas seulement navigation ou page d'accueil | Moyenne | SITE-TOKENS ; contenu/limites identiques, export statique et validation réelle de chaque langue |
| P1-RAM | Publié 0.17.0, validé logiciel/natif : bornes et mesures complètes, 15 Mo non atteints, scénario composé Windows ~43 Mo | Moyenne | Maintien des preuves/limites canoniques ; 30 Mo reste une référence |
| P1-RESTORE | Publié 0.17.0, validé logiciel/natif : propriétaire Windows, états opposés/hotplug/partiel simulés, release natif réel sans devices | Élevée | Pas de certification matérielle ou déni exclusif/ASIO |
| P1-HB | Bloqué en qualification : Cellar ARM group-writable 0775, chaîne source Intel z3 défaillante ; aucune publication du tap | Moyenne | Préfixe sûr sans relâcher contrôles et prérequis Homebrew amont fonctionnels |
| P1-AUR | Validé natif : makepkg/pacman/PipeWire/PTY ; publication AUR bloquée, accès SSH mainteneur non autorisé | Moyenne | Publication du correctif/source épinglée et clé SSH mainteneur AUR autorisée |
| P1-WG | Winget : à faire | Moyenne | Identité MSI et upgrades contrôlés |
| P1-ARM | Linux ARM64 : à faire | Élevée | ABI/sysroot, installation et tests natifs |
| P1-UPGRADE | **Partiel, priorité corrective** : migration privée Windows 0.17.1→0.17.2 + no-op et neuf migrations installer Unix validés ; handoff legacy à consolider. Nouveau défaut réel : redémarrage du tray hérite des flux standard et bloque capture stdout de l'updater ; ancien CLI mappé laisse warning backup | Moyenne | Handoff natif/stdio bornés, receipts/PATH/root/package manager ; pas de kill ni release implicite |
| P2-HOOKS | Retenu, à faire | Élevée | Transitions actives cohérentes, effets bornés |
| P2-NET | Reformulé en métadonnées locales, à faire | Élevée | PID authentifié ; macOS conditionnel |
| P2-LOCK | Reformulé en protection coordonnée, à faire | Élevée | P1-RESTORE et arbitrage des intentions |
| P2-DIAG | Diagnostic retenu ; réparation forcée écartée | Moyenne | Couverture explicite et identité processus |
| P3-STREAM | Flux existant ; adaptateurs à faire | Faible | Contrat événements/santé et versions de schéma |
| P3-PTT | Reformulé en sourdine possédée, à faire | Élevée | P1-RESTORE, intentions et raccourcis OS |
| P3-SOUND | START déjà disponible ; extension à faire | Moyenne | P2-HOOKS : sémantique des transitions |
| P4-ANDROID | Hors cœur, reporté en dernier | Élevée | Projet indépendant et modèle de permissions |
| SITE-ROOT | Demandé, à faire plus tard : ne plus servir l'ancienne page à la racine HTTPS `/` | Faible | Routage Caddy exact ; préserver `/mcw` et les autres services |
| SITE-MCW | Demandé, à faire plus tard : améliorer la présentation et la lisibilité du site `/mcw` | Moyenne | Page statique, parcours Windows/Linux/macOS et vérification desktop/mobile |
| TRAY-WIN | **P0 publié/installé 0.17.2 ; infobulle corrective prioritaire**. Vrai menu 319×262, captures/détails et icône rouge observés sur PC. Infobulle UIA contredit encore l'activité : « aucun accès actif 2 active… » ; ne pas déclarer ce défaut corrigé | Moyenne | Préfixe neutre + résumé fidèle borné ; activité/partial/error/idle, UTF-16 et langues |
| INSTALL-CURL | Livré côté script : étapes/couleurs terminal, résultat et conseils PATH ; vrai curl HTTPS Linux/macOS et DNS validé, aucune activation implicite | Faible | Source immutable et checksum script vérifiés ; Windows conserve MSI/ZIP natif |

## P0-CAM-TRAY — Caméra Zoom non attribuée et croix grise pendant une capture

**Priorité immédiate, demandée le 2026-10-12 :** diagnostic seul initialement ; **reprise du développement autorisée le 2026-10-13**. Correctif désormais **publié en 0.17.2 et paire PC mise à jour après accord « Maintenant »**. Le diagnostic Zoom ci-dessous reste historique : aucun arrêt Zoom/Ktalk, contrôle SDK, changement de profil/permission ou capture média. Le seul redémarrage ultérieur autorisé concerne le tray MCW.

**Constat réel :**

- CLI et menu natif indiquent `0.17.1` ; `mcw tray status` indique `running`. Le vrai menu affiche `État incomplet — détails` et l'icône est une **croix blanche sur fond gris**, réellement capturée : ce n'est pas une supposition ni une ancienne image.
- `mcw status --json` rapporte le micro de `Zoom.exe` actif, signature vérifiée, et `USB 2.0 Camera` active sous `Unattributed camera stream`, `pid:null`, `enforcement:unknown`, `confidence:high`, `risk:suspicious`. **L'utilisateur confirme que Zoom utilise aussi la caméra.** Cette confirmation décrit le scénario réel, mais n'autorise pas MCW à inventer un PID caméra ou à arrêter un processus.
- Le rapport caméra contient `report FILETIME: Some(0)` ; seul `sensor_activity` est `degraded`, les quatre autres collecteurs sont `healthy`. Le code retour `status=1` correspond ici aux accès détectés, pas à une panne générale.
- Le panneau natif **Status details…** du tray dit seulement `miccamwatch: telemetry degraded`, suivi des protections et limites SDK : il ne nomme pas le collecteur dégradé, sa cause, ni les deux accès actifs. Notifications en pause dans le menu ; aucun échec courant de notification ou de contrôle n'apparaît dans ces détails.
- Au relevé : micro `requested:false`, `service_active:false`, `mute_state:unmuted`, deux endpoints ; caméra `desired_blocked:false`, `state:allowed`, un périphérique présent, aucun blocage possédé/restauration en attente/erreur. **Ce relevé ne reproduit pas une protection demandée que Zoom aurait levée.** `doctor --json` retourne 0 ; son avertissement « portable executable » n'explique pas la croix grise.

**Chaîne de défaut vérifiée dans la source publiée :**

1. [`windows_camera.rs`, `ProcessActivity::attributed_pid`](https://github.com/Roman-Cuisset/miccamwatch/blob/dd35f7ddc1bb36c079017f235703175c52d77da5/src/platform/windows_camera.rs#L29-L38) exige `created_at <= reported_at`, en plus de l'identité d'instance. Un report à zéro interdit donc l'attribution à un processus actuel, même si son PID serait valide.
2. [`windows.rs`, `sensor_camera_accesses` et `snapshot`](https://github.com/Roman-Cuisset/miccamwatch/blob/dd35f7ddc1bb36c079017f235703175c52d77da5/src/platform/windows.rs#L683-L789) conservent à juste titre le flux réel, mais sans attribution ; le collecteur devient `Degraded`. Le profil `Strict`, constaté par `doctor`, promeut ensuite `Unexplained` en `Suspicious`.
3. [`tray.rs`, application du snapshot](https://github.com/Roman-Cuisset/miccamwatch/blob/dd35f7ddc1bb36c079017f235703175c52d77da5/src/frontends/tray.rs#L980-L1019) choisit `TrayVisual::Error` dès qu'un collecteur n'est pas `Healthy`, **avant** de résumer les captures actives. Ce chemin produit la croix grise et remplace les accès par le message générique de télémétrie.

**Limites du diagnostic :** l'origine OS/pilote du timestamp nul et le PID brut/HRESULT de `GetProcessId` ne sont pas exposés par ce JSON. Ne pas annoncer que Windows fournit déjà un PID Zoom vérifiable, ni imputer le zéro au pilote sans sonde native métadonnées. [Microsoft documente `GetReportTime`](https://learn.microsoft.com/en-us/windows/win32/api/mfidl/nf-mfidl-imfsensorprocessactivity-getreporttime) comme un FILETIME associé au rapport, sans garantir une convention de timestamp nul. `confidence:high` porte ici sur l'activité observée, pas sur une attribution à Zoom ou un verdict malveillant.

**Correction à réaliser, dans cet ordre :**

1. Qualifier en lecture seule les résultats natifs exacts : PID/HRESULT, streaming, report time, réception du callback et identité/création du processus. Traiter l'horodatage nul comme donnée indisponible, pas comme une date de capture exploitable. Définir une attribution sûre si les garanties natives le permettent ; sinon conserver explicitement l'activité non attribuée. **Ne pas simplement supprimer la garde anti-réutilisation de PID**, remplacer le timestamp par « maintenant », choisir l'application au nom, ou autoriser un kill à partir du seul PID.
2. Corriger le tray : conserver les accès réellement actifs même avec une couverture partielle ; rendre visibles séparément activité, attribution/santé et échec de protection. Les détails doivent nommer le collecteur, la raison et les captures, pas recopier seulement un résumé générique. Ne pas supprimer l'avertissement ni transformer une observation incomplète en état sain.
3. Conserver les protections/règles explicites et la prudence du profil Strict : aucun traitement spécial « Zoom est forcément autorisé ». Ne pas utiliser ce diagnostic pour revendiquer la correction du démutage Ktalk/Audition, non exercé ici.

**Acceptation :** smoke natif avec Zoom utilisant les deux périphériques, comparaison CLI/tray et transitions réelles uniquement avec accord ; caméra attribuée à une instance vérifiée si possible, sinon « active, application non attribuée » avec raison exacte et micro Zoom toujours visible. Couvrir timestamp nul/absent, PID réutilisé/inaccessible, arrêt/rapport vide et collector indisponible ; aucun faux inactif, kill non autorisé ou restauration implicite. Vérifier le retour à une santé complète sans redémarrer le tray ; ne pas généraliser ce défaut Windows aux autres OS sans preuve.

**Preuves privées, non versionnées :** dossier `current-incident-20261012-162622/` sous le dossier de preuves MCW local : `status.stdout.json`, `doctor.stdout.json`, processus MCW, `current-icon.png`, `current-menu.png`, `tray-observation.json`, `native-details.txt`/`.png`. SHA-256 du snapshot `f6c00c89308f6135d700b8ec322414d718f033a7f4af579bdd68aab013989469`, des détails `6ec7cf0f1ac90e8a0d5a7182a110e644939bf8d43f9922e486dfec6f6f4c91ce`, de l'observation tray `65ab897d35bd8c9f4e3c9e3afbcee47d5b05e3185748ffa5434ce25b21b19dbd`. Ne pas publier les snapshots complets contenant les métadonnées de session utilisateur.
### Résultat de la reprise du 2026-10-13

- Collecteur : conserve les résultats/erreurs natifs de `GetProcessId` et `GetReportTime` dans les preuves existantes ; le zéro signifie « horodatage indisponible », jamais une date réelle. Les raisons de refus d'attribution sont explicites : PID absent/erreur, temps absent/zéro/erreur, identité inaccessible/réutilisée, rapport antérieur. Les guards d'instance/fraîcheur restent obligatoires ; schéma JSON 3 inchangé.
- Tray : nouvelle couverture **Partial**, icône orange avec point d'exclamation distincte du pipeline Ready ; première ligne bornée par les comptes, détails avec captures Active/Ready, PID prouvé ou inconnu, collecteurs/raisons, gaps et contrôles. Vraies indisponibilités/erreurs de protection/notification gardent la priorité sans effacer les accès. Refresh sain récupère normalement, sans acquitter une erreur manuelle ni effacer l'historique de notification.
- Local Windows : format, Clippy strict et **167 tests réussis** ; build release et CLI candidate sur les API réelles réussis. Garde timestamp nul/absent, PID zéro/réutilisé/inaccessible, HRESULT d'identité/temps et arrêt/liste vide couverts ; quatre régressions de transitions/précédence du tray.
- Vrai Win32 : popup compact, cinq scénarios natifs dont partial→active récupéré, contenu EDIT complet/wrapping. Les premières captures de détails montraient le fond d'écran et celles d'icône le chevron Explorer : **elles ne prouvent pas ces surfaces**. Harness corrigé : popup réellement à l'écran, fenêtre rendue par `PrintWindow` au HWND, rasterisation du vrai HICON orange puis rouge ; clipboard utilisateur jamais remplacé. Données de stress UI indépendantes, pas présentées comme une capture matérielle.
- **Zoom réel lancé par l'utilisateur pendant son créneau de cinq minutes :** CLI candidate détecte micro et caméra actifs ; sonde C++ directement liée au SDK Microsoft confirme streaming=1, `GetProcessId=S_OK`, PID de `Zoom.exe` et création d'instance concordants, mais **`GetReportTime=S_OK` avec FILETIME=0**. Ce n'est donc pas une erreur de conversion des bindings Rust. La cause précise du zéro côté fournisseur/capture reste inconnue ; aucun pilote accusé. Sans fraîcheur garantie, la caméra reste explicitement non attribuée/Unknown, jamais un PID deviné autorisant un kill.
- Ensuite, le watcher candidate `--json --no-kill` a observé des START micro et caméra **Ktalk**, avec un report FILETIME caméra valide et attribution d'instance réussie. Ne pas généraliser le zéro à toutes les applications ; **aucun STOP Zoom n'a été observé dans ce watcher**, commencé ensuite et arrêté par sa borne de temps. Protection micro non demandée/caméra Allow : aucun test de réassertion mute ou de déni physique revendiqué.
- Preuves privées : `p0-camera-20261013-070021/`, CLI/sonde/identité native Zoom et manifeste d'intégrité. SHA-256 du relevé Zoom candidate `e6fb50d590db0ea9d65791f441a067045b11fed1b98fdd4807f820f8fe4f757a`, sonde SDK `04d5e65dd992322a7e98b2e7fd48d3eb8a986fec44395dc0d931120c48b9e705`. Ne pas publier snapshots/PIDs/ancêtres utilisateur.

- [CI exacte `233398024fa822b6ed99748971bd272d60e3d7d2`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38355075760) entièrement verte : Windows, Linux, macOS ARM et Intel, build/tests/audit, vrais frontends/packages, popup/détails/Copy all sur clipboard runner isolé, gardes Windows zéro-device et MSI install/uninstall. Source intégrée sur `main`, **aucun nouveau tag/release ou swap de l'installation PC** ; ces preuves n'autorisent pas à supprimer le refus d'attribution sans timestamp sûr.

### Livraison coordonnée 0.17.2 et smoke PC réel

- Source immuable `a58c8b9f3a967589a576879d6d27bfdbc40f9dff`, tag annoté `02442ba59b0bc49e5c838e18266096c69b876cfa` ; [CI pré-tag](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38361454701) et [release native](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38364052525) quatre OS réussies. Sept assets téléchargés : taille/digest API concordants ; six payloads/SBOM concordent aussi avec leur entrée nommée SHA256SUMS et leur attestation workflow/ref/source/signer contraints, runners hébergés. SHA256SUMS ne se contient pas lui-même. EXE/compagnon/MSI **NotSigned** ; aucune identité Authenticode inventée.
- Migration privée Windows publique 0.17.1→0.17.2 réellement exécutée, exit 0, paire exacte et no-op exit 0 avec hashes/mtimes inchangés. Préférences natives et copie readonly de l'historique conservées ; vrai tray utilisateur étranger à ce scope inchangé. Warning OS error 5 de nettoyage de l'ancien CLI encore mappé conservé sans bypass ; ce scénario n'exerçait pas le restart GUI.
- [Migrations publiques Unix 38366922327](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38366922327), source/tag exacts : Linux/macOS ARM/Intel × bash/zsh/fish, neuf chemins **installer managed** 0.17.1→0.17.2, PATH/état utilisateur conservés. Mac : vrai bootstrap public 0.16.0→0.17.2, puis vrai `./mcw update` HTTPS no-op. Le test in-place corrigé utilise son client privé explicitement identifié, pas une réparation de l'ancien client public. PipeWire headless sans inventaire physique ; aucune certification capture/Gatekeeper/autostart activé.
- Après second choix explicite **« Maintenant »**, protections revalidées et updater PC lancé une seule fois. Paire finale exacte ZIP public : CLI SHA-256 `d9c52c03fdf437dec91f3c3a590b7a73a44e20c76ca6eab47afb6d3c0600fb2b`, compagnon `7dd5571fd545d0072812825ecd4f98b0728b0b8b8736a082c3f27bce888d4c72`. Ancien compagnon remplacé par un nouveau PID/instance, pas de tray CLI legacy présent à ce relevé. Capture stdout/exit de l'updater indisponible après timeout du kernel ; **aucune seconde mise à jour**. `--version`, hashes, processus et GUI récupérés readonly prouvent le résultat, pas un exit inventé.
- Vrai menu **0.17.2, 319×262 pixels**, rendu propriétaire `PrintWindow` ; deux captures Ktalk (Mic/Cam), un pipeline Edge Ready explicitement non prouvé capture dans les détails. Bouton MCW identifié par UIA puis vrai crop shell : **point d'exclamation rouge**, aucune croix grise. Première capture menu à l'écran montrait WPS au premier plan : non utilisable, jamais preuve. Aucun clipboard remplacé ; seul panneau de détails créé par le smoke refermé.
- Cinq collecteurs healthy ; protection snapshot **strictement égal avant/après** : micro non demandé/garde inactive/unmuted, caméra Allow, aucun pending/unknown/helper actif. Ce constat n'est pas une reproduction du démutage sous protection demandée, ni un déni exclusif/ASIO ou une certification physique. Le test Zoom zéro et transitions native Partial→Active précédents restent les preuves de ce chemin, pas un Zoom relancé implicitement.
- Preuves privées non versionnées sous `release-0172-20261013-090127/` : assets/intégrité/attestations, migrations Windows/Unix, `installed-pc-0172/` et rendu/icon/détails natifs. Ne pas publier snapshots de session utilisateur.

### Défauts résiduels prioritaires observés pendant la livraison

1. **TRAY-WIN — infobulle fausse** : UIA lit réellement « miccamwatch : aucun accès actif 2 active (1 Mic, 1 Cam); 1 ready… », alors que CLI/menu/détails montrent les captures. Le littéral inactif existe dans `src/frontends/tray.rs::idle_text` ; les références hors workspace n'ont pas été résolues par LSP, ne pas inventer une chaîne d'appels. Correction attendue : nom produit neutre devant résumé borné réellement courant ; inactivité uniquement si état idle. Qualifier active/partial/error/idle, langues et limite UTF-16, sans élargir le menu ni masquer la santé.
2. **P1-UPGRADE — flux hérités après restart** : `src/updater/process.rs::restart` fait `Command::spawn` sans configurer stdin/stdout/stderr ; ces flux sont hérités. Lors du vrai handoff, nouveau tray vivant/paires 0.17.2, plus de CLI updater observé, mais `subprocess.run(capture_output=True)` ne rend pas sa sortie ; timeout 210 s et kernel détruit. **[INFERENCE]** l'EOF est retenu par le compagnon héritier, cohérent avec la source et l'absence de ce problème dans le smoke privé sans restart. Correction à qualifier sur runner desktop jetable : détacher uniquement les trois flux du compagnon, capturer la fin de l'updater avec nouveau tray toujours vivant ; refus/protections/identité/restoration inchangés. Ne pas contourner en tuant le tray utilisateur, en cachant timeout/erreur ou en libérant une garde.


## Phase 1 — Packaging et écosystème de distribution (immédiat)

### P1-RAM — Qualifier la mémoire et borner les accumulations avant extension

**Fonction :** établir une baseline puis supprimer les croissances non bornées avant d'élargir le produit. Priorité aux observations et aux effets existants, pas à une nouvelle abstraction de performance.

**Modules Rust :** `src/platform/linux/pipewire.rs`, `src/watcher/windows.rs`, `src/watcher/unix.rs`, `src/frontends/tray.rs`, `src/frontends/tray/unix.rs`, `src/platform/macos.rs` ; `Cargo.toml`/`Cargo.lock` pour contrôler les dépendances.

**Contraintes historiques de la base, avant correction candidate :** le JSON PipeWire autorisait **16 MiB** de brut puis un `Vec<serde_json::Value>` ; cette limite n'était pas une borne RSS. `stderr` du subprocess n'était pas borné. Le tray Windows avait des canaux non bornés et un thread par notification ; le cooldown du watcher Windows n'était pas purgé. Ces faits de code constituaient des risques, pas une mesure de dépassement. Borner sans tronquer silencieusement les captures : dépassement = santé dégradée/indisponible explicite. Ne pas supprimer les preuves d'identité pour économiser la mémoire.

**Livré en 0.17.0 :** lecture PipeWire streaming bornée,
stdout/stderr et données retenues bornés, deadlines enfant et erreurs de santé
explicites sans faux STOP ; files UI/notifications/caches bornés. `top` conserve
une seule rangée horizontale de six boutons, avec représentation compacte aux
petites largeurs. Cela corrige les risques historiques, sans certifier le RSS.

**Preuves locales et CI native :** comparaison Windows locale `fe86151`/stable
sur machine peuplée et [CI complète `078cdc7`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38047504453)
terminées ; Windows/Linux/macOS ARM/Intel, garde/tray/menu, gros graphe PipeWire,
rafales et lacunes explicites. **15 Mo non atteints.** Les gardes Windows sans
conhost sont mesurés, pas estimés ; chiffres RAM/CPU/latence et limites dans la
[table canonique](ARCHITECTURE.md#local-candidate-evidence-and-remaining-qualification).
Le runner vide ne remplace pas la machine locale ; pas d'affirmation sous 15 Mo.

**Validation :** baseline stable/pic et latence sur les trois OS, gros graphes et rafales d'événements, UI lente, enfant bloqué/sortie excessive ; aucune accumulation et aucun STOP inventé en cas de limite. Toute impossibilité de respecter le budget doit être remontée avant ajout de fonctions, pas compensée par une mesure plus flatteuse.

**Complexité : Moyenne**, potentiellement Élevée si le profiling impose de modifier le parsing ou le cycle des helpers.

### P1-RESTORE — Préserver chaque microphone Windows

**Fonction :** rendre la restauration après verrouillage sûre pour plusieurs endpoints et préparer lockdown/PTT.

**Modules Rust :** `src/platform/windows.rs`, `src/frontends/tray.rs`, `src/collector.rs`, `src/model.rs` ; réutiliser les conventions de possession de `src/platform/linux/control.rs` et `src/platform/macos/control.rs`, sans remplacer leurs backends.

**Contraintes historiques :** dans v0.16.0, le tray sauvegardait `mute_state == Muted` dans un seul booléen. Un état `Mixed` devenait donc `false`, ensuite appliqué à tous les endpoints : risque de démuter une entrée initialement coupée. Les originaux par endpoint du backend servaient seulement au rollback immédiat. Exigence conservée : ID stable, original, génération/intention et readback ; une action manuelle récente prime sur une restauration tardive. Déconnexion, nouvel endpoint, redémarrage et échec de rollback ne doivent pas devenir un succès implicite.

**Livré en 0.17.0 :** propriétaire natif de l'intention
manuelle micro jusqu'au release explicite, indépendant de `top`/tray ; originaux,
générations et tokens automatiques par endpoint, priorité manuelle et propriété
release-pending en cas d'échec. Intention demandée, mute SDK effectif et capture
observée restent séparés. Aucun service login/autostart implicite. Linux/macOS
restent one-shot dans leurs portées approuvées.

Le GUID de contexte CoreAudio est consultatif ; `SetMute` même valeur peut rendre
`S_FALSE` sans callback. Attribution exacte Ktalk/Audition et toute intention
externe invisible ne sont pas garanties : pas de frontière de sécurité absolue.
Caméra Windows : Block global même à zéro/1 000 appareils, helper temporaire UAC
pour arrivées futures ; Allow uniquement possédé, journal admin protégé, pas
d'import legacy non signé. Recovery `camera allow --restore-legacy INSTANCE_ID`
= nouvelle autorisation UAC/class validation mono-cible, pas exception Logitech.
Fenêtre brève d'arrivée, restart/veto/unknown restent explicites. IPC fixe lié
SID/session/data-dir hash et génération native, borné, sans chemins/devices
arbitraires ; délégation admin alternatif QUERY-only.

**Preuves :** Clippy strict feature, deux suites Windows (161 lib +1 main,
1 visuel ignoré), build release et [CI native complète](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38047504453)
réussis ; vrais propriétaires Windows sans devices, IPC fini, réservations
update, test ACL admin→medium, TUI sept langues, 120/150/40/20 colonnes et
souris/refresh/quit. La nouvelle TUI locale est également réussie ; aucun SDK
micro modifié. Le recovery legacy caméra déconnectée approuvé a retiré l'ancien
flag, puis status medium exit 0 sans nouvelle UAC ni dette restante.
Release explicite et restauration achevée exigés avant remplacement ; jamais
de release implicite pour updater. La restauration physique reste non certifiée.

**Sécurité update 0.17.0 (tests natifs locaux et CI native réussis) :** portable Windows
réserve request locks et ressources natives micro/caméra avant stop/swap,
refuse propriétaire actif/étranger, intention/token, release-pending ou journal
non fiable. Readiness strictement read-only sans release/helper/SDK. Réservation
swap/rollback, réacquisition avant rollback post-restart ; protection reprise
= rollback refusé et backups gardés. No-op paire identique sans réservation de
remplacement. MSI externe/manuelle ne passe pas par ces gardes du portable ;
release explicite reste la procédure, pas garantie d'enforcement MSI.
L'intention globale Block et les reçus caméra protégés requested/owned/unfulfilled
bloquent. L'historique legacy non signé `restore_on_arrival` owed-only, seul, ne
bloque pas un remplacement autrement permis : préservé inchangé pour recovery
explicite, il ne devient pas une autorité élevée.

**Validation :** deux entrées avec états opposés, changement manuel entre lock/unlock, hotplug et échec partiel ; seules les modifications possédées sont restaurées exactement. Pas de promesse de coupure électrique ou de déni d'accès WASAPI.

**Complexité : Élevée.** Correction 0.17.0 publiée et validée en logiciel/natif ; limites physiques explicites, releases antérieures immuables.

### P1-HB — Formule Homebrew macOS

**Fonction :** installation, mise à niveau et suppression par Homebrew pour macOS 15+ ARM/Intel. Commencer par un tap maintenu, puis soumettre à Homebrew/core si ses critères sont satisfaits ; « officiel » ne signifie pas acceptation déjà obtenue.

**Modules Rust :** pas de collecteur supplémentaire ; contrats de `build.rs`, `src/platform/macos.rs`, `src/updater/unix.rs` et `src/autostart/unix.rs`. Le helper Swift/AppKit existant reste embarqué ; sa compilation exige le SDK/compilateur adéquat, pas un nouveau runtime applicatif.

**Contraintes :** formule versionnée, source/artefacts immuables et checksums ; recettes de build avec dépendances verrouillées. Homebrew possède le préfixe, pas le receipt de l'installateur curl : pas de double updater ni de remplacement des fichiers du gestionnaire par `mcw update`. Valider la cible de LaunchAgent après upgrade sans écraser une inscription externe ou activer l'autostart. Aucune modification Gatekeeper/quarantaine et aucune signature Developer ID fictive.

**Validation :** `brew install/upgrade/uninstall`, CLI et menu natif, conservation des préférences/historique et de l'intention d'autostart sur ARM/Intel.

**Implémentation en qualification :** `distribution/homebrew/Formula/miccamwatch.rb`,
fixture publique 0.16.1 et `native_qualification.py` ; workflow
`homebrew-native.yml`, ARM/Intel × loaded/dormant/disabled. Vrais
install/upgrade/cleanup/test/uninstall/reinstall, menu AppKit, conservation des
préférences/historique et refus des inscriptions externes ; postinstall absent
sans activation ni receipt. L'amont 0.17.1 résout et valide l'exécutable physique
du Cellar ; refresh possédé avant nettoyage de l'ancien keg, préservation
loaded/dormant/disabled et no-op courant sans redémarrage.
Source 0.17.0 historique immuable, pas de patch de formule pour masquer le défaut.
Publication exige le digest observé du correctif et le tap maintenu
`Roman-Cuisset/homebrew-miccamwatch`, puis un vrai install depuis ce tap ;
Homebrew/core et Developer ID/notarisation ne sont pas revendiqués.

**Complexité : Moyenne.**

### P1-AUR — Paquet Arch Linux

**Fonction :** `PKGBUILD` et `.SRCINFO` maintenus, construits depuis un tag immuable avec Cargo verrouillé. Pas de deuxième variante binaire/source tant qu'elle n'est pas nécessaire.

**Modules Rust :** contrats de `src/platform/linux.rs`, `src/platform/linux/pipewire.rs`, `src/updater/unix.rs`, `src/autostart/unix.rs`, `src/privacy/linux.rs` ; scripts existants de `installer/` et patch ABI `vendor/libspa` à conserver/évaluer explicitement.

**Contraintes :** dépendances PipeWire/SPA/`pw-dump` déclarées ; outils desktop/sons seulement pour les usages associés. Le package manager possède ses fichiers. L'installation du paquet ne doit ni activer l'autostart ni installer/autoriser silencieusement le helper root et sa policy Polkit. Les chemins privilégiés et receipts actuels ne deviennent pas une seconde convention AUR : payload reviewable et bootstrap administrateur explicite séparés ; tout changement de chemin exige une migration propre.

**Validation :** construction en environnement Arch propre, installation/upgrade/removal, PipeWire réel et PATH ; absence d'action caméra/root et préservation des fichiers utilisateur.

**Implémentation en qualification :** `distribution/aur/PKGBUILD`, `.SRCINFO` et
`qualify-native.py`, workflow `arch-package-native.yml`. Makepkg ordinaire,
Cargo fetch verrouillé puis build frozen ; patch `vendor/libspa` conservé,
providers PipeWire/SPA/pw-dump déclarés. Container Arch natif x64 sans devices,
pas de privilèges ou sockets hôte. Pacman install/reinstall/upgrade **pkgrel**
et removal ; vraie session PipeWire privée/D-Bus et PTY ; strace du refus
updater avant Internet, byte-preservation des settings/history/autostart externe.
Cette mise à niveau de révision n'est pas une migration de version amont.
Payload root simplement consultable sous `/usr/share/miccamwatch/linux-camera` ;
bootstrap séparé avec archive release et SHA-256 fiables, jamais tarball source
ou package pacman substitué. AUR nécessite un accès SSH mainteneur vérifié ;
nom libre au RPC du 2026-10-11, aucune soumission ou acceptation officielle.

**Complexité : Moyenne.**

### P1-WG — Manifeste Winget Windows

**Fonction :** soumettre le MSI x64 per-user existant à `winget-pkgs` avec identifiant stable, URL de version et SHA-256 exacts ; le ZIP reste le canal portable, pas un second installeur concurrent.

**Modules Rust :** contrats de `src/updater.rs`, `src/updater/transaction.rs`, `src/autostart.rs` et `src/platform/windows.rs` ; packaging `installer/miccamwatch.wxs` et `.github/workflows/release.yml`. Pas de client Winget intégré.

**Contraintes :** conserver l'UpgradeCode MSI et la portée per-user ; valider détection, upgrade, downgrade refusé et uninstall. Pour MSI/Winget, upgrades via le gestionnaire : le remplacement portable ne met pas à jour l'enregistrement MSI. Ne pas présenter le conseil actuel comme une protection déjà imposée par l'updater ; compléter le refus de double propriété si nécessaire. Signature seulement avec certificat réel, sans contournement SmartScreen/Defender. Acceptation communautaire du manifeste séparée de la publication GitHub.

**Validation :** validate du manifeste, installation/upgrade/désinstallation réelle, CLI/tray de même version, PATH et autostart antérieur préservés.

**Complexité : Moyenne.**

### P1-ARM — Linux ARM64 natif et Raspberry Pi/SBC

**Fonction :** produire `aarch64-unknown-linux-gnu` et `miccamwatch-linux-aarch64.tar.gz` ; la cross-compilation prépare le paquet, les smokes ARM64 natifs en autorisent la publication.

**Modules Rust :** `src/platform/linux.rs`, `src/platform/linux/pipewire.rs`, `src/platform/linux/control.rs`, `src/platform/linux/procfs.rs`, `src/privacy/linux/usb.rs`, `src/privacy/linux/helper.rs`. Intégration : `Cargo.toml`, `vendor/libspa`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `.github/workflows/unix-install.yml`, `installer/install.sh`, `installer/install-camera-helper.sh` et `installer/tests/`.

**Contraintes :** sysroot/compiler, bindgen et pkg-config doivent viser les bibliothèques/header ARM64 de la baseline, sans emprunter ceux de l'hôte x64. Maintenir glibc 2.35+ et l'ABI PipeWire/SPA documentée ; aucune promesse ARMv7/OS 32 bits/musl. L'installateur ordinaire, le bootstrap root et les tests sont actuellement x64-only ; le bootstrap vérifie aussi l'ELF `e_machine=62`. Adapter la validation complète à AArch64 (`183`), sans supprimer les contrôles d'intégrité/architecture ni les restrictions de tests privilégiés aux machines jetables.

**Validation :** fmt/clippy/tests, vraie extraction et CLI/PTY sur ARM64, serveur PipeWire, installation/migration/uninstall ; smoke desktop uniquement avec session/panel réels. Une VM ou QEMU ne prouve pas le support matériel d'un Pi. Tester un SBC autorisé séparément : micro observable et, si disponible, webcam USB conforme. Les caméras CSI/non-USB restent hors contrôle USB ; un SBC headless n'exige pas un tray fictif.

**Complexité : Élevée.** Nouvelle plateforme : release mineure plutôt que patch cosmétique.

### P1-UPGRADE — Consolider installation et chemins de mise à niveau

**Fonction :** garder un seul contrat curl/PATH et une matrice de migration explicite entre archives gérées, installations de package manager, ZIP/Cargo et MSI. La migration publique 0.16.0 → 0.16.1 est validée ; conserver cette origine et celles encore documentées pour les prochains lots.

**Modules Rust :** `src/updater.rs`, `src/updater/unix.rs`, `src/updater/download.rs`, `src/updater/transaction.rs`, `src/autostart.rs`, `src/autostart/unix.rs`, `src/settings.rs` ; `installer/install.sh`, bootstrap root et tests natifs.

**Contraintes :** résoudre latest une fois, vérifier le paquet exact avant exécution, ownership et remplacement sur le même volume, interruption/rollback et erreurs de quarantaine visibles. PATH bash/zsh/fish idempotent avec consentement, jamais de `sudo` curl. Préserver préférences, historique et autostart opt-in ; ne pas mélanger receipts curl et propriété Homebrew/AUR/MSI. Maintenir l'ordre Linux : restore avec ancien helper, uninstall root explicite, upgrade utilisateur, nouveau bootstrap explicite. Un état caméra Allowed ne remplace pas un journal vide.

**Lot correctif 0.16.1 :** mise à jour en place d'un `mcw` macOS extrait hors `bin`, avec bootstrap explicite de l'ancien client, hash/identité/propriétaire verrouillés et quarantaine conservée. Les layouts `bin` sans receipt ne sont pas assimilés à des archives portables. Ce lot ne livre pas Homebrew/AUR ni Linux ARM64 et ne clôture pas la consolidation. Il inclut aussi des menus tray bornés et diagnostics complets séparés sur les trois OS, plus les boutons adaptatifs de `top` ; aucune nouvelle capacité de contrôle n'est annoncée.

**Validation :** download public réel, origine 0.15.1 via installateur courant, 0.16.0 vers nouveau paquet, origine Windows portable/MSI, cible en cours d'exécution, chemins avec espaces, erreurs réseau/digest/permissions et interruption. Pas de shim ni de mutation des assets/tags anciens.

**Complexité : Moyenne.**

## Phase 2 — Défense active et extensibilité légère (moyen terme)

### P2-HOOKS — Hooks locaux de transitions de capture

**Fonction :** ajouter à la politique TOML des déclencheurs `on_capture_start` et `on_capture_stop`, désactivés par défaut : exécutable local absolu + arguments littéraux. Variables bornées `MCW_PID`, `MCW_PROCESS_INSTANCE`, `MCW_EXECUTABLE`, `MCW_DEVICE`, `MCW_RESOURCE` et instant d'observation ; champs inconnus absents, jamais PID inventé. Les scripts utilisateur envoient Telegram/webhooks/IoT ; mcw ne connaît aucun fournisseur.

**Modules Rust :** `src/config.rs`, `src/model.rs`, `src/watcher.rs`, `src/watcher/unix.rs`, `src/watcher/windows.rs`, `src/frontends/tray.rs`, `src/frontends/tray/unix.rs` ; futur petit module d'exécution dédié, pas dans le helper root ni dans `notify_access`.

**Contraintes :** unifier seulement la réconciliation nécessaire, aujourd'hui distincte sous Windows watch/tray/TUI. Une transition Ready→Active est actuellement UPDATE ; les hooks suivent le cycle **Active**, pas les noms d'action aveuglément. Aucun hook de démarrage sur baseline initiale/récupération par défaut, aucun arrêt déduit d'une lacune, d'un filtre de risque, de Ready ou de la fermeture du programme. Séparer événements informatifs et éligibilité des effets ; une caméra macOS peut fournir un événement sans PID.

Un worker paresseux, un enfant maximum, file bornée en nombre/octets, deadline monotone et récolte à la fermeture. Débordement explicite ; pas de STOP isolé si son START n'a pas été accepté, pas de retries/exactly-once implicites ni de garantie de restitution d'un effet externe après crash. Ne pas coalescer START/STOP comme un refresh UI. Environnement minimal documenté, répertoire maîtrisé, pas d'héritage aveugle des secrets/variables loader, stdout/stderr hors du JSON et bornés ou nuls. Pas de shell construit avec les données ; un interpréteur peut être choisi explicitement par l'utilisateur. Refuser l'exécution depuis un mcw élevé. Un hook exécute du code aux droits utilisateur : **ce n'est pas une sandbox** ; groupes Unix/Job Objects Windows ne donnent pas une garantie portable contre tous descendants malveillants.

**Validation :** aucune exécution option désactivée ; Active/Ready, lacunes/recovery, PID absent/réutilisé, noms malveillants, rafales, enfant bloqué/descendants et arrêt du watcher. Watch et tray partagent le contrat ; status et peinture TUI ne deviennent pas des lanceurs de scripts.

**Complexité : Élevée.** La réconciliation et la maîtrise des effets dominent le coût du TOML.

### P2-NET — Corrélation locale instance de processus/sockets

**Fonction :** sur demande et pour une instance authentifiée Active, afficher sockets TCP/UDP IPv4/IPv6 visibles, endpoints numériques, fraîcheur et couverture. Une IP « suspecte » ne peut venir que d'une règle locale explicite ; présence simultanée capture/socket ne prouve ni trafic sortant ni exfiltration.

**Modules Rust :** `src/model.rs`, `src/collector.rs`, `src/platform/mod.rs`, `src/platform/windows.rs`, `src/platform/linux.rs`, `src/platform/linux/procfs.rs`, `src/platform/macos.rs`, `src/frontends/cli.rs`, `src/output.rs`, `src/frontends/tui.rs` ; extensions conditionnelles de `native/macos_process.swift` et du bridge existant.

**Contraintes OS :**
- Linux : joindre FD `socket:[inode]` de `/proc/<pid>/fd` aux tables du namespace pertinent (`/proc/<pid>/net/tcp{,6}` et `udp{,6}`). Ces tables décrivent le namespace, pas les sockets du seul PID. Vérifier starttime/identité avant/après ; hidepid, FD/inode réutilisé, sockets partagées et conteneurs donnent une couverture partielle, pas une fausse absence. Pas de scan de tous les processus ni de `setns`/eBPF/root implicite.
- Windows : `GetExtendedTcpTable` et `GetExtendedUdpTable` avec propriétaire PID ; activer uniquement les features natives IpHelper/WinSock nécessaires dans la crate `windows` existante. Bornes/alignment/tailles FFI et naissance du processus revérifiée ; le PID OS seul n'est pas une identité stable.
- macOS : priorité au micro/PID explicitement vérifié ; caméra CoreMediaIO sans PID = corrélation indisponible. Pas de polling permanent `lsof`. Évaluer l'introspection FD via le helper existant seulement après revue du SDK : [le header libproc Apple avertit que ses interfaces privées peuvent changer](https://github.com/apple-oss-distributions/xnu/blob/main/libsyscall/wrappers/libproc/libproc.h). Si aucun contrat maintenable n'est retenu, garder le diagnostic `lsof` ponctuel dans un script externe plutôt que promettre un backend universel.

Aucun packet capture, DNS inverse, réputation cloud, GeoIP ou client réseau. UDP peut ne pas exposer de pair distant. Buffers réutilisés, limites de tuples/octets et cache court par identité/namespace, option désactivée sans travail supplémentaire ; aucun arrêt automatique sur la seule IP.

**Validation :** sockets réelles contrôlées, IPv6/UDP, processus terminé/PID réutilisé, droits refusés, namespace distinct et identité absente ; limites explicites et budget P1-RAM respecté.

**Complexité : Élevée** au global ; Moyenne pour snapshot Windows ciblé, Moyenne à Élevée Linux, Élevée/conditionnelle macOS.

### P2-LOCK — `mcw lockdown`, protection coordonnée à portée explicite

**Fonction :** demander ensemble la sourdine des entrées compatibles et le blocage des caméras couvertes, avec bilan **appliqué / partiel / en attente d'autorisation / non pris en charge / échec** par ressource. Ne pas annoncer une coupure globale instantanée.

**Modules Rust :** `src/frontends/cli.rs`, `src/main.rs`, `src/collector.rs`, `src/model.rs`, `src/settings.rs`, frontends tray/TUI, `src/platform/windows.rs`, `src/platform/linux/control.rs`, `src/platform/macos/control.rs`, `src/privacy.rs`, `src/privacy/linux.rs`, `src/privacy/macos.rs`.

**Contraintes :** Windows = signal WASAPI + caméras présentes/UAC ; Linux = signal PipeWire de la session + USB/UVC couvert/Polkit et helper installé ; macOS = INPUT writable + profil caméra à approuver manuellement. Pas de déni ALSA/autres sessions, de coupure matérielle, de hotplug global ou de mutation TCC. Approbation peut prendre du temps ou être refusée ; aucun résultat global positif à partir d'une seule action réussie. L'ensemble n'est pas atomique entre drivers.

Après P1-RESTORE, définir une seule intention partagée entre CLI/tray/TUI, générations et priorité : protection/lock explicite ne doit pas être annulée par un PTT ou une restauration périmée. Restaurer uniquement les originaux possédés et préserver les dettes d'échec ; réutiliser les commandes de restauration existantes, sans « tout autoriser ». Ne pas élargir le protocole root à l'exécution de commandes libres. `Unknown` de verrouillage n'autorise aucune action.

**Validation :** approbation/cancellation réelle, couverture partielle, état initial déjà coupé/bloqué, actions concurrentes, hotplug et reprise après erreur. Expliquer que l'isolation matérielle exige un dispositif physique.

**Complexité : Élevée.**

### P2-DIAG — Périphérique occupé, pas de `release-locks` magique

**Fonction :** enrichir `mcw doctor` et les explications avec faits et limites : capture active, FD seulement présent, permission refusée, processus disparu, contrôle non writable, blocage possédé ou politique externe. **Écarter la commande qui prétend libérer automatiquement les verrous matériels orphelins.**

**Modules Rust :** `src/frontends/cli.rs`, `src/main.rs`, `src/model.rs`, `src/output.rs`, `src/frontends/tui.rs`, `src/platform/windows.rs`, `src/platform/unix.rs`, `src/platform/linux/v4l2.rs`, `src/platform/linux/procfs.rs`, `src/platform/macos.rs` et modules privacy existants.

**Contraintes :** un zombie a déjà terminé et libéré ses FD ; le tuer ne débloque pas une caméra. Un FD V4L2 ouvert est Ready, pas preuve de streaming ou de culpabilité. Caméra macOS sans PID : aucun propriétaire deviné. Conserver fermeture normale puis arrêt **ciblé, confirmé et lié à l'instance** déjà disponible dans la TUI ; PID seul, processus protégé/autre utilisateur ou identité incertaine = refus. Un signal accepté n'est pas preuve que la sortie/I/O est achevée ni que le driver est réparé. Pas de fermeture de handles étrangers, reset de pilote, unload global, redémarrage du serveur audio ou destruction du journal caméra.

**Validation :** erreurs de permission, processus disparu, FD concurrent, source indisponible et PID réutilisé ; message de résultat sans faux « verrou libéré ». Le diagnostic actuel `explain` porte sur les observations de capture, pas sur tous les processus système.

**Complexité : Moyenne.** Réparation forcée cross-OS exclue.

## Phase 3 — Intégrations desktop minimalistes (ergonomie)

### P3-STREAM — Barres d'état par le flux JSON existant

**Fonction :** documenter et fournir de petits adaptateurs externes Waybar/Polybar/Sway à partir de **`mcw watch --json`** et du snapshot `status --json`. La proposition `status --format json-stream` est écartée comme doublon ; aucun second poller/daemon/plugin chargé dans le cœur.

**Modules Rust :** contrats de `src/frontends/cli.rs`, `src/main.rs`, `src/output.rs`, `src/model.rs`, `src/watcher/unix.rs`, `src/watcher/windows.rs`. Modifications Rust seulement si un défaut du contrat existant bloque une intégration réelle.

**Contraintes :** consommateur valide le schéma, distingue événements et documents santé, gère snapshot initial, recovery et état inconnu. Ne pas transformer une ligne vide/déconnexion en idle ; ne pas faire un `mcw status` subprocess à chaque seconde si le flux suffit. stdout machine sans textes/hook output, stderr diagnostic et arrêt de pipe géré proprement. L'adaptateur produit le format propre à la barre et échappe noms/markup ; ses dépendances restent externes. Étudier la cohérence santé Windows/Unix sans renommer silencieusement toutes les sorties `--json`.

**Validation :** vraie barre Linux, capture/start/stop et perte du backend, schéma inconnu, lecteur lent/fermé, markup hostile ; mémoire bornée et pas d'enforcement depuis une icône.

**Complexité : Faible** pour adaptateurs/documentation ; Moyenne si une correction du contrat de santé est nécessaire.

### P3-PTT — Push-to-talk logiciel possédé et raccourcis natifs

**Fonction :** mode opt-in : sourdine des entrées compatibles au repos, restauration admissible pendant l'appui, retour à la sourdine possédée au relâchement, originaux restaurés à la sortie du mode. Ce n'est ni un contrôle matériel universel ni une réservation du micro à une application.

**Modules Rust :** `src/settings.rs`, `src/collector.rs`, `src/platform/windows.rs`, `src/platform/linux/control.rs`, `src/platform/macos/control.rs`, `src/frontends/tray.rs`, `src/frontends/tray/unix.rs`, `src/frontends/tui.rs` ; helper `native/macos_desktop.swift` et IPC existant.

**Contraintes :** P1-RESTORE et arbitrage P2-LOCK indispensables. Ne pas ouvrir un micro initialement coupé, annuler une intention manuelle récente ou réutiliser un ancien ID de source. Tous les clients de la source peuvent profiter de la fenêtre d'ouverture ; aucun enforcement par application ni absence garantie de fuite temporelle. Événement release perdu, repeat, session suspendue ou raccourci perdu doivent donner une transition possédée sûre et un état visible.

Windows : `RegisterHotKey` seul n'offre pas un protocole PTT key-up complet ; valider appui/relâchement. Linux : portail GlobalShortcuts si supporté/autorisé, via `zbus` déjà présent, ou raccourci configuré par le bureau avec IPC explicite ; jamais capture générique Wayland/lecture evdev privilégiée. macOS : mécanisme natif revu dans le helper, permissions éventuelles demandées explicitement ; pas de bypass TCC. La TUI actuelle traite Press seulement : ne pas vendre ses touches comme raccourci global.

**Validation :** clavier/session réelle pour chaque backend revendiqué, conflits de raccourcis, release manquant, lock/PTT/action manuelle, entrées pré-muettes et hotplug. En l'absence d'API fiable, annoncer non disponible plutôt que simuler un PTT universel.

**Complexité : Élevée.**

### P3-SOUND — Earcons natifs optionnels, sans moteur audio

**Fonction :** étendre le son START actif déjà présent avec une petite liste fermée de sons système discrets par ressource et, sur demande explicite, STOP confirmé. Préférences désactivées par défaut ; pas de banque audio embarquée ni de fichiers audio à analyser.

**Modules Rust :** `src/settings.rs`, `src/watcher/unix.rs`, `src/watcher/windows.rs`, `src/platform/unix.rs`, `src/platform/windows.rs`, `src/platform/macos.rs`, frontends tray ; `native/macos_desktop.swift` et validation du protocole helper.

**Contraintes :** réutiliser `canberra-gtk-play`, `NSSound` AppKit et sons système Windows. START existe mais cadence, pause/cooldown et chemins tray ne sont pas identiques entre OS ; aligner les effets sur les transitions éligibles P2-HOOKS, notamment Ready→Active et lacunes. Worker borné pour éviter que son/notification ne bloquent l'observation. Pas de thread par bip, de fallback synthétique ou de nouveau lecteur audio. Respecter pause/préférences et disponibilité du service/thème ; absence de son audible n'est pas une preuve de panne de collecte.

**Validation :** transition réelle, cooldown séparant START/STOP, pause, device inconnu, service audio absent et enfant bloqué. Écouter sur session utilisateur réelle ; CI headless ne certifie pas l'audibilité ni l'autorisation des notifications.

**Complexité : Moyenne**, incluant cohérence watch/tray/OS ; faible variation de son seule insuffisante pour déclarer le lot livré.

## Phase 4 — Hors-périmètre du binaire et non-goals

### P4-ANDROID — Satellite `mcw-android`, strictement indépendant et dernier

**Fonction :** étudier une application Kotlin utilisant des APIs Android autorisées et, si pertinent, Shizuku opt-in ; distribution F-Droid à instruire dans son propre dépôt/pipeline. Aucun backend Android factice dans le desktop.

**Modules Rust impactés : aucun.** Le modèle JSON desktop peut servir de référence documentaire, pas de raison d'ajouter JNI, Android ou un runtime Kotlin aux dépendances de miccamwatch.

**Contraintes :** permission utilisateur, disponibilité/mode Shizuku, différences Android/OEM et critères F-Droid à qualifier. Shizuku ne garantit pas un inventaire de toutes les captures par processus ni un contrôle universel caméra/micro. Pas de contournement de permission, root imposé ou compatibilité fictive ; identité, signature et lifecycle propres au satellite. N'engager ce chantier qu'après stabilisation/qualification des phases desktop.

**Validation :** matrice Android/OEM sur appareils autorisés, refus/révocation des permissions, Shizuku absent/redémarré et pipeline F-Droid réel. Les preuves desktop ne valident rien ici.

**Complexité : Élevée.**

### Non-goals formels

| Exclusion | Description et frontière | Modules Rust / contraintes | Complexité |
| --- | --- | --- | --- |
| Analyse spectrale/ultrasons | Aucune écoute permanente ni analyse d'échantillons audio | Aucun ajout dans `collector`/`platform` ; observation de métadonnées uniquement, pas de nouvelle permission capture ni stockage de signal | Élevée si entreprise ; volontairement exclue |
| Productivité/temps de parole | Aucun score de productivité ni métrique de durée de parole | Ne pas détourner `history`/`model` vers l'analytique ; timestamps = observations, pas parole mesurée | Moyenne si entreprise ; exclue |
| Clients réseau lourds | Aucun SDK Slack/Discord/Telegram/IoT, client de réputation ou webhook intégré | Intégrations uniquement scripts/hooks et JSON standard ; updater/trust natifs existants restent des besoins distincts, pas un prétexte à étendre la surveillance réseau | Moyenne à Élevée si entreprise ; exclue |

## SITE-PAGES — Site MCW complet sur Cloudflare Pages, priorité urgente

**Demande utilisateur :** réaliser au plus vite un vrai site de présentation de
MCW, complet et bien fini, que l'utilisateur puisse mettre sur **Cloudflare
Pages**. Priorité du prochain chantier web ; ne pas attendre la fin de toutes
les phases de packaging pour le commencer. **État : livré statique le 2026-10-13,
prêt pour Pages ; aucun déploiement Cloudflare revendiqué.**

**Livrable :** un site statique autonome prêt à publier sur Pages, avec fichiers
et instructions de déploiement exactes (dossier de sortie, commande de build si
nécessaire, variables réellement requises). Pas de serveur applicatif, runtime
Node côté visiteur, dépendance obligatoire au serveur ASUS ou secret dans le
contenu public. Domaine `pages.dev` fonctionnel ; domaine personnalisé facultatif,
avec instructions DNS adaptées sans modifier les autres services.

**Contenu attendu :** présentation claire de miccamwatch, bénéfices concrets,
captures réelles du tray/TUI, fonctionnement et fonctions effectivement publiées ;
parcours Windows/Linux/macOS distincts, prérequis, téléchargement et installation,
commande curl Unix copiable avec résultat et conseils PATH, méthode Windows
native ; démarrage, usage quotidien, désinstallation, mises à jour et dépannage ;
FAQ, limites de protection/attribution, confidentialité, signature et permissions
UAC/Polkit/TCC ; liens dépôt, documentation et releases. Ne pas présenter les lots
de roadmap ou recettes non publiées comme disponibles ni promettre un blocage
universel. Pas de remerciement répétitif dans la présentation.

**Présentation et qualité :** hiérarchie lisible, identité visuelle cohérente,
navigation simple, boutons d'action alignés et adaptatifs, captures utiles plutôt
que décoratives, contenu lisible sur téléphone et desktop. Accessibilité clavier,
contrastes, focus visible, texte alternatif, respect du mouvement réduit ; pages
légères, sans trackers ajoutés par défaut, métadonnées SEO/social et page 404
propres. Downloads et exemples suivent la release réellement publiée.

**Validation :** déployer réellement sur un projet Pages autorisé ou, sans accès
au compte Cloudflare, livrer et servir le dossier statique puis consigner le
prérequis exact de publication. Vérifier le vrai rendu desktop/mobile, navigation
clavier, liens/téléchargements, copie des commandes, HTTPS et routes/404. Exécuter
le flux curl Unix annoncé dans un environnement isolé ; aucune activation
implicite des protections, de l'autostart ou du tray. Ne pas annoncer le site
déployé sur la seule base d'une compilation.

**Articulation :** ce site Pages est une demande distincte. SITE-ROOT/SITE-MCW
ci-dessous restent différés ; ne pas supprimer la racine ASUS ou refondre `/mcw`
implicitement. Le contenu et l'installateur revus peuvent être réutilisés sans
dupliquer leur contrat ou imposer le port ASUS à la navigation Pages.

### SITE-TOKENS — Placeholders visibles, correction urgente

**Signalement utilisateur :** `__MCW_VERSION__`, `__MCW_SOURCE__` et
`__MCW_INSTALLER_SHA256__` apparaissent au lieu de la version, du commit source
et de la somme du script. Défaut utilisateur établi ; **pas encore corrigé**.
Les smokes antérieurs ne le réfutent pas.

**Évidence source :** `site/index.html` et `site/404.html` sont des templates ;
`site/build.mjs::renderText` substitue les valeurs de `site/release.json` et
refuse les marqueurs non résolus. Le chemin exact actuellement consulté/servi
n'est pas établi : ne pas accuser un upload utilisateur, un cache ou Cloudflare
sans preuve. Examiner source HTML directe, export, ZIP livré et publication
effective pour corriger le vrai parcours, pas seulement une copie locale.

**Correction attendue :** version réellement publiée, source immuable exacte,
SHA-256 de l'installateur effectivement servi ; source unique des valeurs,
pas de remplacement arbitraire ou de duplication de pins. Garantir que le
parcours de prévisualisation/publication ne présente pas les templates bruts
comme un site prêt à utiliser. Faire échouer une livraison contenant des tokens
dans ses pages, métadonnées, liens ou commandes.

**Acceptation :** vrai rendu de la livraison concernée, accueil et 404, téléchargements,
curl, source et provenance avec valeurs correctes ; aucune occurrence résiduelle
de marqueur `__MCW_*__` dans les ressources destinées aux visiteurs. Vérifier le
ZIP effectivement livré et, si déployé, les vrais bytes/URLs HTTPS publics.
Le checksum doit correspondre aux bytes du script, non à une archive.

### SITE-L10N — Site complet dans toutes les langues MCW

**Demande utilisateur urgente :** couvrir exactement les langues déclarées par
`src/i18n.rs::Language` : **anglais `en`, français `fr`, allemand `de`,
espagnol `es`, japonais `ja`, chinois simplifié `zh`, russe `ru`**.
État actuel : site français ; **traductions complètes non livrées**.

**Périmètre :** toutes les sections, navigation, parcours Windows/Linux/macOS,
installation/usage/mise à jour/désinstallation, FAQ, sécurité/confidentialité,
limites d'attribution et de protection, légendes/texte alternatif, labels ARIA,
retours de copie, 404 et métadonnées SEO/social. Traduire le sens sans embellir
les capacités ; une limitation ne doit pas disparaître dans une langue.
Ne pas traduire/altérer les hashes, commits, noms de fichiers, URLs ou syntaxe
des commandes. Les screenshots réels gardent leur langue et leur portée
historiques ; légendes traduites, aucune image maquillée comme capture localisée.

**Présentation :** choix de langue visible, utilisable au clavier et sur mobile,
liens directs par langue et navigation fonctionnelle sans JS ; aucune redirection
forcée qui empêche le choix. `lang`, canonical/hreflang/sitemap cohérents lorsque
la vraie origine de production est connue, sans domaine inventé. Réutiliser le
layout/build statiques ; aucun framework ou service de traduction distant,
tracker ou runtime supplémentaire côté visiteur.

**Acceptation :** contenu complet et relu dans chacune des sept langues, sans
section laissée silencieusement en français/anglais ni clé/token affiché.
Exercer réellement chaque langue sur desktop/mobile, navigation clavier/sans JS,
sélecteur, liens, 404 et commandes copiables ; vérifier textes longs et glyphes
CJK/cyrilliques sans débordement. Les faits de release/source/checksum restent
identiques et corrects dans tous les exports ; SITE-TOKENS doit être résolu.


### Livraison statique du 2026-10-13 et publication

- Sources `site/` : site français complet, CSS/JS externes, captures Windows réelles avec portée, installations MSI/ZIP/Linux/macOS, curl et review-first, provenance/non-signature, usages/permissions/FAQ/confidentialité et vraie 404. Pas de framework/npm/Node côté visiteur, tracker, miroir de binaire ou dépendance ASUS.
- Export **`node site/build.mjs` → `site-dist/`**, qualifié sous Node 24. Données publiées `site/release.json` : 0.17.1/source immuable `dd35f7d`, SHA installer `b7348c9a6f37a3d4f89faa2b767fd4880162d90c8f5d7c151185e0823384d1bb`. Export refuse checksum/origine invalide, output non possédé/symlink ; préserve fichiers étrangers. Six scénarios sandbox réels passés, ancienne sortie conservée sur erreur.
- ZIP prêt : **`Téléchargements/miccamwatch-pages-0.17.1.zip`**, 172 596 octets, 13 fichiers, `index.html` à la racine ; SHA-256 `2668ef934e285a20cf77f91f5fe87a3f3ec25c1dac848e4bb6d76546ba0abb24`. Aucun secret, binaire MCW ou manifeste privé de preuve dans ce ZIP.
- Runtime officiel Wrangler Pages 4.149.0 : desktop 1366×900, mobile 390×844 et sans JS 320×720 ; images chargées, pas d'overflow/ancres cassées/IDs dupliqués, aucun sous-ressource tiers ni erreur JS. Onglets OS au clavier, focus visible, contenu de tous les OS sans JS ; copie refusée/texte sélectionné testés sans toucher au clipboard utilisateur. Succès d'écriture clipboard système non exercé.
- HTTP réel Pages : ressources/MIME, route profonde inconnue **404**, script `text/plain` et digest exact ; CSP restrictive et `Permissions-Policy` interdisant micro/caméra. Vrai curl HTTPS annoncé exécuté sur Linux dans HOME/préfixe temporaires, version native 0.17.1, checksum script et uninstall possédé réussis ; aucun root/helper Polkit, tray, autostart ou contrôle activé.
- Publication directe : Cloudflare **Workers & Pages → Create application → Get started → Drag and drop your files → Deploy site**, déposer le ZIP ou contenu de `site-dist/`. [Guide officiel](https://developers.cloudflare.com/pages/get-started/direct-upload/). Direct Upload ne se convertit pas ensuite en Git integration ; choisir un projet Git séparé pour le build automatique.
- Git integration : racine dépôt, framework none, commande `node site/build.mjs`, output `site-dist`, `NODE_VERSION=24` pour reproduire. `MCW_SITE_URL` = vraie origine HTTPS de production ; sinon `CF_PAGES_URL` si fourni. Sans origine, canonical/URLs social absolues/sitemap sont omis sans domaine inventé. Domaine personnalisé facultatif ; ne pas remplacer le routage ASUS.
- Prérequis restant : projet/compte Cloudflare autorisé et upload. Après déploiement, vérifier son vrai HTTPS, commande same-origin `/install.sh`, canonical/social/sitemap et clipboard selon permissions du navigateur. La preuve locale HTTP n'est pas une preuve de publication HTTPS.
- Caller installer ASUS corrigé **uniquement** sur métadonnées version/source/checksum : ancienne page 0.17.0 annonçait un hash incompatible avec le script 0.17.1 déjà servi. Les pins sont cohérents et vérifiés via HTTPS ; SHA de la racine inchangé. Aucune refonte `/mcw`, suppression racine ou modification/restart Caddy ; SITE-ROOT/SITE-MCW restent différés.

### Actualisation après publication 0.17.2

- `site/release.json` et recettes épinglent désormais la vraie source `a58c8b9f3a967589a576879d6d27bfdbc40f9dff`/release 0.17.2. Archive source 820 983 octets SHA-256 `bda453c43ae9cc98c34757eecec615f287365ca7b6ec785d9aeb638b91626dc1` ; [Arch 38362707189](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38362707189) makepkg/pacman/PipeWire/PTY/install/reinstall/pkgrel-upgrade/remove réussis. Homebrew/AUR publication restent bloqués, aucune permission ou dépendance amont relâchée.
- Nouvelle archive **`Téléchargements/miccamwatch-pages-0.17.2.zip`**, **137 898 octets**, 13 fichiers, index à la racine ; SHA-256 `570d96a6f1805753b36aa704af1d0283608cef7c6add4a89b686ab96eb2d4158`. Ancienne archive 0.17.1 conservée. Carte sociale dans la palette verte du site, artwork produit 0.17.2 ; screenshots natifs toujours 0.17.1 et légendés historiques, pas relabellés.
- Vrai export Node + Wrangler Pages 4.149.0 : nouveau rendu desktop 1366×900 et mobile 390×844, images chargées, sans overflow ; nouvelle FAQ partielle exercée. HTTP réel : index 200, script 51 249 octets/SHA `b7348c9a6f37a3d4f89faa2b767fd4880162d90c8f5d7c151185e0823384d1bb`/text/plain, image sociale 200, route inconnue profonde 404 et en-têtes CSP/Permissions-Policy. Preuves clavier/no-JS/clipboard refusé de la première livraison restent historiques, pas réexécutées ni attribuées à ce passage.
- ASUS : seuls pins HTML version/source migrés vers 0.17.2/a58, script inchangé et HTTPS strict vérifié ; hash racine avant/après identique `484e4b60bd6cf549f9c79aec6e073327c306b3245adaa87f6aeb48c97391371a`. Vrai `curl | sh` HTTPS du DNS, latest 0.17.2 native Linux puis uninstall possédé, HOME/XDG temporaires et `--no-modify-path` ; aucun root/helper, autostart, tray ou contrôle activé.
- Cloudflare non déployé : projet/compte autorisé et upload restent le prérequis exact. Sans origine, canonical/social absolus/sitemap omis ; après upload vérifier vrai HTTPS et clipboard. SITE-ROOT/SITE-MCW toujours différés ; pas de Caddy/routes/restart/refonte.



## Site HTTPS — demandes différées

**État : demandé par l'utilisateur, à faire plus tard.** Cette inscription au
suivi n'autorise aucune modification du site maintenant ; aucune des deux
demandes ci-dessous n'est annoncée comme réalisée.

### SITE-ROOT — Ne plus charger l'ancienne page à la racine

**Demande :** retirer la page actuellement accessible à
`https://serveur-asus.duckdns.org:9443/` pour qu'elle ne se charge plus.
La racine exacte `/` ne doit plus servir cette interface ni ses contenus, et
ne doit pas rediriger vers elle ou vers `/mcw`. Prévoir une réponse sans page,
par exemple HTTP 404 avec corps vide ; pas une nouvelle page d'accueil.

**Contraintes :** limiter la modification à l'exposition HTTP de cette racine ;
ne pas supprimer les fichiers, données ou services sous-jacents hors périmètre.
Conserver TLS, le port 9443 et le fonctionnement de
`https://serveur-asus.duckdns.org:9443/mcw`, de `/mcw/` et de `/mcw/install.sh`.
Ne pas casser les autres routes/services non concernés.

**Validation future :** GET HTTPS strict de `/` sans ancien contenu ni
redirection ; `/mcw` reste utilisable, éventuellement via sa redirection
canonique vers `/mcw/` ; page et installateur accessibles, script inchangé et
checksum vérifié. Vérifier les autres routes concernées avant/après.

### SITE-MCW — Améliorer le site sans perdre son fonctionnement

**Demande :** conserver `/mcw` fonctionnel mais améliorer la page actuelle,
jugée mal présentée par l'utilisateur. Revoir la hiérarchie visuelle, la
lisibilité, l'espacement et la navigation ; rendre le choix Windows/Linux/macOS,
les téléchargements et les instructions d'installation faciles à trouver.
Alléger la lecture de la page sans supprimer les limites ni les précautions.

**Contraintes :** rester sur le site statique existant, sans framework/runtime
ou service réseau supplémentaire. Garder les versions et liens réels,
checksums/provenance, avertissements de signature et limites de protection/RAM.
Ne pas confondre installateur Unix et packages Windows ; ne pas proposer de
contournement antivirus/quarantaine ou de téléchargement non vérifié.

**Validation future :** observer le vrai rendu desktop et mobile, navigation
clavier et contraste lisibles, sans débordement horizontal global ; essayer les
liens, le téléchargement du script et les instructions propres à chaque OS.
Ne pas déployer cette refonte dans le cadre de la seule mise à jour du MD.

## Livraison Git, releases et suivi à chaque lot majeur

1. **Avant modification :** fixer le lot, ses critères ci-dessus, la matrice OS touchée, les limites de preuve et le budget ressources. Ne pas attendre la fin d'une phase entière pour corriger un défaut de sécurité indépendant.
2. **Validation :** reproduire un bug avant correction ; garder une régression consumer-visible pertinente. `cargo fmt --check`, Clippy avec `--locked` et `-D warnings`, tests natifs et dépendances auditées ; features `windows-tray`/`linux-camera-helper` là où nécessaires. Pas de tests qui ne vérifient que la copie du wiring ou le texte source. Un target check cross ne remplace pas un smoke natif.
3. **Smoke réel :** paquet extrait, CLI/tray/TUI concernés, installer et upgrade depuis la release précédente, rollback/uninstall et permissions refusées. Matériel/autorisation indisponibles = limite nommée, jamais faux succès. Mesures RAM/CPU/latence publiées pour les modes touchés.
4. **SemVer décidé par le lead :** patch `0.16.x` pour correction compatible, hardening ou packaging sans nouvelle capacité publique ; prochaine mineure pour nouvelle plateforme, hooks, contrôles ou contrat public. En pré-1.0, une rupture de schéma/protocole exige une mineure et une migration explicite. Les recettes de distribution peuvent cibler 0.16.0 sans reconstruire artificiellement ses binaires. Pas de version future figée avant validation du contenu.
5. **Git :** commits propres du lot et de sa documentation, push sur `main` après revue/CI ; préserver les changements locaux utilisateur, secrets et diagnostics bruts. Aucun force-push de tag publié, asset remplacé ou alias de compatibilité permanent.
6. **Release officielle :** mettre à jour les versions cohérentes, taguer le commit vérifié, déclencher le workflow existant ; notes indiquant portée, migration, preuves et limites. Artefacts Windows/Linux/macOS complets, nouvelles architectures revendiquées incluses, SPDX, checksums et attestations. Stable/latest seulement avec Windows approuvé et build réussi ; sinon prerelease explicitement sélectionnée, pas une stable incomplète. Ne jamais inventer une signature Windows ou une notarisation macOS.
7. **Vérification publique :** API latest/tag, ensemble d'assets, hashes/provenance et vraie installation/migration depuis les téléchargements publiés. Pour les gestionnaires, distinguer recette soumise, acceptée et réellement installable ; un refus externe n'est pas une publication officielle réussie.
8. **Mise à jour vivante :** modifier les lignes du tableau et le journal ci-dessous à chaque lot. La validation avant tag entre dans le commit de release ; les observations obtenues après publication entrent dans un commit documentaire ultérieur, **sans déplacer le tag**. Documenter le reste à faire, les limites OS et le prochain lot ; ne pas rouvrir artificiellement ce qui est livré.

## Journal de suivi

| Date | Lot | État / livraison | Preuves | Reste à faire |
| --- | --- | --- | --- | --- |
| 2026-10-06 | Baseline v0.16.0 | Publiée, hors nouvelles phases | [CI main](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37439356665), [release native](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37437092551), [migration 0.14.0](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37438076601), [migration 0.15.1](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37438076504) | Limites matérielles/OS dans Architecture ; aucune affirmation RAM <15 Mo |
| 2026-10-06 | Roadmap | Plan technique établi, aucune phase produit clôturée | Modules et contrats relus dans la base 0.16.0 ; propositions arbitrées ci-dessus | Démarrer par P1-RAM/P1-RESTORE ; packaging puis autres lots selon dépendances |
| 2026-10-06 | Patch 0.16.1 updater/tray/top | [Livré stable/latest](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.16.1) | [CI complète](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37487806861), [release native](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37493346742), [migration publique 0.16.0](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37495128116) : Windows/Linux/macOS ARM/Intel ; menus Win32 560/562 px à 125 % de DPI, XFCE 399/398 px Latin/CJK, AppKit 240 pt sur 7 langues et diagnostics longs, détails complets. TUI : 7 langues, souris/clavier et redimensionnements. Bootstrap macOS public et updater corrigé, présence/absence de quarantaine, SIGTERM/rollback exact, changements concurrents préservés. Sept assets publics, hashes et six attestations vérifiés ; vrai updater Windows 0.16.0 → 0.16.1 puis no-op | Consolidation P1-UPGRADE et autres phases restent ouvertes ; pas de mesure RAM <15 Mo, de signature ni de nouvelle preuve matérielle |
| 2026-10-09 | Candidat source 0.17.0 Windows-first P1-RAM/P1-RESTORE | En cours, non publié ; stable/latest reste v0.16.1 | Bornes et propriétaire natif implémentés ; [CI native `fe86151`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38037123382) Windows/Linux/macOS ARM/Intel verte, propriétaires natifs sur Windows sans devices, PipeWire virtuel, ressources et limites dans Architecture | Modifications suivantes encore à qualifier ; aucune preuve physique microphone ou déni exclusif/ASIO, aucun autre lot clôturé |
| 2026-10-10 | Détachement console et recovery legacy autorisé | Validé logiciel/natif, non publié ; récupération ciblée achevée | [CI native `078cdc7`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38047504453) verte Windows/Linux/macOS ARM/Intel ; gardes sans conhost, EOF CLI correct, malformed peers, release explicite et cleanup ; test ACL admin→medium ; UAC acceptée, ancien flag caméra déconnectée 1→0, reçu fulfilled, client medium exit 0 ; aucune mutation micro/capture, ACL inchangées ; 161 lib +1 main, TUI sept langues locale et CI | Intégrer main et qualifier avant tag ; publier et vérifier les sept assets/provenance puis migrations publiques ; physical/ASIO non certifiés, 15 Mo non atteints, autres lots ouverts |
| 2026-10-10 | Intégration main et qualification UI + deux gardes | Main intégré ; nouvelle qualification composée en cours, pas de tag | Source `72a0bdd` sur main ; baseline ZIP publique 0.16.1 fraîchement téléchargée, hash vérifié et version native réussie dans un dossier privé ; camera status 0.17 profil neuf réussi ; harness étendu top/tray avec deux gardes actives et contrôle des rôles dans chaque sample | CI main et phases composées avant tag ; fenêtre propre non observée localement, tray utilisateur existant `running` laissé intact (singleton global dans la source) ; ne pas additionner des médianes de phases séparées pour annoncer un total composé |
| 2026-10-10 | Contrôles de référence avant publication | v0.17.0 toujours non publiée ; aucun tag créé | [CI main `72a0bdd`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38049013635) : Windows/Linux/macOS Intel verts, macOS ARM échoue sur **la baseline publique 0.16.1**, `top` en rafale n'ayant pas quitté dans les 20 s ; comparaison interrompue avant le candidat, preuve conservée. Vérificateur public exercé sur 0.16.1 : sept assets/hashs et six attestations du workflow/ref/source exacts vérifiés ; updater natif privé 0.16.1→0.16.1 exécuté deux fois, octets/mtime/préférences et processus exact du tray utilisateur étranger préservés | Qualification composée `6847482` en cours ; ne pas annoncer la CI main précédente complètement verte ni déduire un résultat candidat de l'échec de référence ; migration 0.16.1→0.17.0 seulement après publication réelle |
| 2026-10-10 | Qualification différenciée et probe foreign-scope | Corrections de harness, aucun changement de contrôle matériel ; requalification native composée requise | `6847482` : référence macOS ARM 0.16.1 Quit échoue à nouveau ; Windows s'arrête avant phases composées sur status 2 du profil étranger, cleanup réussi. Probe étranger réutilise maintenant la validation stricte des données de protection et du zéro-device avec codes status 0/1/2. Gate candidat explicite : erreurs fonctionnelles de référence conservées/averties, tous les profils candidat obligatoires ; comparaison non qualifiée si référence échoue ; intégrité/isolation/sampler/cleanup restent fatals. Smoke Windows readonly réel : baseline et candidat, status/watch/top/rafale, tous réussis ; RAM 15 Mo non atteints. Refus réel d'un tray utilisateur existant confirmé fatal avant lancement | Requalifier les deux phases UI + gardes sur runner vide et tous les targets ; aucun tag ni migration vers 0.17.0 avant publication vérifiée |
| 2026-10-10 | Qualification composée et candidat complet | Validé logiciel/natif avant publication ; source `32f87cd`, pas encore de tag | [CI main complète](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38051530685) Windows/Linux/macOS ARM/Intel verte ; quatre comparaisons ressources complètes et qualifiées baseline/candidat, anciens échecs conservés. Deux gardes + top/tray réellement concurrents : rôles présents dans tous les samples, Quit/WM_CLOSE préservent les protections, release explicite et cleanup sans erreurs. Médianes 42,820/43,516 Mo, pics de fenêtre 43,098/43,721 Mo ; gardes seules 27,922 Mo, pic scénario complet 56,041 Mo. Tableau RAM/CPU multi-OS et limites dans Architecture | Commit de validation documentaire puis tag/release ; vérifier sept assets/hashs/six attestations et migrations publiques avant annonce stable. 15/30 Mo non atteints en scénario composé ; matériel, ASIO/exclusif et autres lots non certifiés/clôturés |
| 2026-10-10 | Arrêt tray macOS avant tag | Nouveau défaut candidat découvert, publication bloquée | [CI `016df87`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38053174757) Windows/Linux/macOS Intel verts ; macOS ARM baseline entière et profils candidat hors tray réussis, mais stop tray candidat échoue. Gate candidat reste faux, aucune publication. Source : readiness AppKit partagée effacée par le serveur stop avant acceptation par la boucle, interleaving pouvant produire une fausse déconnexion ; reply exact non conservé dans ce run | Correction de la transition sans masquer les vraies déconnexions ; assertion native stop inchangée et diagnostics de contrôle bornés ajoutés ; requalifier natif avant nouveau commit vérifié/tag, aucun autre lot rouvert |
| 2026-10-10 | Requalification après correction arrêt AppKit | Validé logiciel/natif avant tag ; source `29db092` | [CI native complète](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38055262096) Windows/Linux/macOS ARM/Intel verte. Arrêts tray réels Linux/macOS : contrôle stop 0 et sortie processus 0, cleanup sans échec. Quatre comparaisons baseline/candidat complètes et qualifiées ; garde + top/tray Windows requalifiés, rôles présents, protections conservées et cleanup réussi. L'échec candidat précédent est conservé, non accepté | Commit de validation avant tag puis publication/provenance/migrations publiques ; 15/30 Mo composés et qualification physique toujours non acquis ; aucune modification de l'installation/tray utilisateur |
| 2026-10-10 | Livraison P1-RAM/P1-RESTORE 0.17.0 | [Publié stable/latest](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.17.0), tag annoté immuable source `d1b8557f827e102df20c33dbac4ec99badba7d1a` | [CI exacte pré-tag](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38056837640), [release native](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38058344725), sept assets/digests et six attestations exactes vérifiés ; Windows public 0.16.1→0.17.0, paire ZIP/préférences/no-op et tray utilisateur étranger préservés ; [migrations publiques Unix](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38059262974) Linux/macOS ARM/Intel, managed 0.16.1→0.17.0 et bootstrap Mac depuis public 0.16.0, limites de fixture explicitement conservées ; DNS page/script/TLS/desktop/mobile/curl réel vérifiés ; source utilisateur inchangée | 15 Mo non acquis, scénario composé ~43 Mo et pics explicites ; physique/exclusif/ASIO non certifiés, EXE/MSI NotSigned, aucune nouvelle clearance AV/Apple ; Homebrew/AUR/Winget/Linux ARM64 et autres lots ouverts. Preuves post-publication dans commit ultérieur sans déplacer le tag |
| Demande utilisateur après livraison 0.17.0 | SITE-ROOT / SITE-MCW | Consigné, différé ; aucune modification du site | Racine HTTPS à ne plus servir ; `/mcw` à conserver fonctionnel et à améliorer visuellement ; critères et limites dans la section Site HTTPS | Réaliser ultérieurement les deux demandes, puis vérifier HTTPS/routes/script et vrai rendu desktop/mobile |
| 2026-10-11 | SITE-PAGES | **Priorité urgente consignée, à réaliser au plus vite** ; aucun déploiement effectué | Demande utilisateur : site MCW complet, soigné et publiable sur Cloudflare Pages ; contenu, livrable, accessibilité et critères de validation décrits ci-dessus | Réaliser le prochain chantier web sans attendre tout le packaging ; prouver le rendu et les parcours ; accès Cloudflare requis uniquement pour revendiquer un déploiement réel |
| 2026-10-11 | TRAY-WIN / P1-HB / P1-AUR / INSTALL-CURL | Correctifs en cours, non publiés ; stable reste 0.17.0 | Local Windows : Clippy/tests/build et popup Win32 compact prouvés. Première qualification : [CI](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38146600418) refuse le nouveau numéro dans le gate caméra ; [Homebrew](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38146994580) bloque avant candidat sur alias historique group-writable ARM et bouteilles Intel manquantes ; [Arch](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38146600485) expose bindgen/Clang 23, pas un changement de layout SPA | Gate version revu, cible physique validée sans relâcher permissions, prérequis Homebrew Intel réellement construits, bindgen amont corrigé et roundtrip C/Rust ; requalification native nécessaire. Installer curl : afficher étapes et commande sans runtime supplémentaire ou activation ; vrais flux HTTPS par OS requis avant livraison |
| 2026-10-11 | Correctif 0.17.1 / INSTALL-CURL / packaging | Source `dd35f7d` validée ; tag annoté créé, publication native en cours ; script DNS livré | [CI exacte](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38152020817) quatre OS verte ; [installer Unix](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38152020898) trois OS vert, curl réel ; [Arch](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38152020811) install/reinstall/pkgrel-upgrade/remove, PipeWire/PTY et user-state validés. DNS script 51 249 octets SHA-256 `b7348c9a6f37a3d4f89faa2b767fd4880162d90c8f5d7c151185e0823384d1bb`, vrai terminal Linux coloré et uninstall isolé réussis | Vérifier artefacts/provenance publics et migrations après disponibilité ; Homebrew bloqué sur `/opt/homebrew/Cellar` 0775 et formule amont z3 Intel en erreur ; AUR `Permission denied (publickey)` après empreinte officielle vérifiée. Aucun arrêt/libération des protections Windows, changement Caddy/racine/page ou site Cloudflare déployé |
| 2026-10-12 | Livraison corrective 0.17.1 | Publié stable/latest ; script DNS et commandes courantes livrés, installation PC laissée intacte | [Release native](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38210407613) verte ; tag annoté `32f8e1398238e8ec7b6fde4c072efaa3516b8dc1`, source `dd35f7d`, archive SHA-256 `cb6da0697df60e4f5a5ff3286154b29b9eee6ab994c5710a04726fbbe64c2575` ; sept assets API/SHA256SUMS et six attestations workflow/ref/source contraints vérifiés ; CLI publique Windows réelle 0.17.1, compagnon inspecté sans lancement ; EXE/MSI non signés. Vrai DNS curl latest 0.17.1 et uninstall isolé réussis ; [migrations publiques Unix 0.17.0→0.17.1](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38212488807) trois OS et [Arch source release exacte](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38212488566) validés | Lecture seule PC : 0.17.0, capture active, protections actuellement non demandées ; aucun arrêt UI/capture ni mutation SDK. Accord avant bref redémarrage UI, readiness native revalidée avant swap. Site Pages urgent consigné ; Homebrew/AUR externes toujours bloqués, aucun faux statut publié |
| 2026-10-12 | Déploiement PC 0.17.1, approuvé « Maintenant » | Livré et smoke réel réussi | Updater natif : paire exacte du ZIP public ; ancien tray CLI 0.17.0 conservé au premier swap, version réellement lue dans son menu ; fermeture coopérative native avec PID/ack exacts puis nouveau compagnon 0.17.1 ; menu 356×262 pixels à DPI 120, icône sans croix grise ; cinq collecteurs healthy, intentions inchangées micro non demandé/unmuted et caméra Allow ; aucun kill Ktalk ni commande SDK ; backup mappé retiré après sortie ancienne UI | Dernier snapshot sans accès actif, pipeline prêt visible : ne pas l'attribuer à notre update ni annoncer un déni matériel. Ancien message « tray not running » du stop CLI malgré GUI legacy présent consigné pour P1-UPGRADE ; pas de contournement ACL/AV, aucun prototype laissé installé |
| 2026-10-12 | P0-CAM-TRAY | **Diagnostic urgent établi sur PC ; correctif non appliqué**, priorité avant le site à la demande utilisateur | Vrai tray 0.17.1 : croix grise, menu « État incomplet », détails « telemetry degraded ». CLI : micro Zoom actif, caméra active non attribuée avec report FILETIME zéro, sensor_activity degraded ; utilisateur confirme la caméra Zoom. Gate timestamp et branche tray masquant les captures relus dans la source publiée ; aucune mutation SDK/redémarrage/arrêt Zoom | Qualifier PID/HRESULT natifs et attribution sûre sans affaiblir l'identité ; préserver les captures dans le tray et détailler la santé partielle ; smoke réel et transitions autorisées avant de déclarer corrigé |
| 2026-10-13 | P0-CAM-TRAY | Correctif source validé local Windows, non publié/installé | Clippy/167 tests/build/SDK CLI, vrai popup et rendu HWND/HICON partiel→actif ; vrai Zoom micro+caméra : PID natif valide, FILETIME zéro avec S_OK, attribution prudente ; Ktalk observé avec timestamp valide. Aucune mutation SDK/arrêt d'app/clipboard ; limites de premières captures explicites | CI/source puis release coordonnée ; pas de preuve de déni physique ni de démutage corrigé, installation publique 0.17.1 préservée |
| 2026-10-13 | SITE-PAGES / INSTALL-CURL caller | Site complet statique livré, ZIP prêt ; pas de déploiement Pages | Vrai runtime Pages desktop/mobile/no-JS/clavier/404/en-têtes, checksum installer ; curl HTTPS Linux réellement exécuté/installé/versionné/désinstallé sous HOME privé. ZIP 172 596 octets/13 fichiers ; pins page ASUS réparés minimalement, racine inchangée | Déposer ZIP/projet Pages autorisé, puis contrôle HTTPS/canonical/same-origin et clipboard ; SITE-ROOT/SITE-MCW toujours différés |
| 2026-10-13 | Qualification source P0-CAM-TRAY / SITE-PAGES | **CI native complète réussie**, source `233398024fa822b6ed99748971bd272d60e3d7d2` intégrée main ; pas de nouvelle release | [CI 38355075760](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38355075760) Windows/Linux/macOS ARM/Intel entièrement verte, TUI/tray/détails/copy natifs, packages/gardes/MSI et migrations Mac ; preuve locale Zoom/SDK indépendante conservée | Release coordonnée distincte avant remplacement du PC ; upload Pages autorisé puis contrôle HTTPS, aucune publication/installation inventée |
| 2026-10-13 | Préparation release 0.17.2 | Version coordonnée en qualification, stable publique reste 0.17.1 ; PC non modifié | Cargo/lock produit 0.17.2, allowlist smoke Linux revue sans relâcher gardes, notes release P0 ; 167 tests locaux et build/native CLI réelle 0.17.2 réussis. Captures site restent explicitement 0.17.1 historiques | Qualifier source exacte avant tag immuable ; vérifier sept assets/sommes/six attestations et migrations publiques, puis site/recettes ; accord moment UI si conférence active |
| 2026-10-13 | Release publique 0.17.2 / migrations / recettes | **Publié stable/latest et qualifié** ; source `a58c8b9`, tag annoté immuable | CI pré-tag 38361454701, release 38364052525 quatre OS ; sept assets/sommes/six attestations exactes ; vraie migration Windows privée + no-op, Unix 38366922327 neuf migrations installer 0.17.1→0.17.2 + bootstrap Mac, Arch 38362707189 natif | EXE/MSI non signés, matériel non certifié ; Homebrew/AUR publication restent bloqués |
| 2026-10-13 | PC 0.17.2 / P0-CAM-TRAY | **Paire publique installée, vrai tray observé après accord « Maintenant »** | Hashes exacts, nouveau compagnon, menu 319×262/version172/accès/détails et vrai glyphe shell rouge ; cinq collecteurs healthy, protection JSON égal avant/après, Ktalk jamais arrêté, aucun contrôle SDK | Timeout de capture updater, exit/stdout non observés ; priorité P1-UPGRADE flux hérités et TRAY-WIN préfixe idle contradictoire. Pas de nouvel essai Zoom ni démutage protégé revendiqué |
| 2026-10-13 | SITE-PAGES / caller ASUS | **Livraison statique 0.17.2 prête, non déployée** | ZIP 137 898 octets/13 fichiers/SHA `570d96a6f1805753b36aa704af1d0283608cef7c6add4a89b686ab96eb2d4158`, rendu desktop/mobile/FAQ/HTTP/404 ; metadata ASUS HTTPS172, racine identique et vrai curl latest172/uninstall isolé | Upload Pages autorisé puis vrai HTTPS/canonical/clipboard ; captures171 historiques ; SITE-ROOT/SITE-MCW différés |
| 2026-10-13 | Dossier projet principal | **Synchronisé dans `C:\Users\stcui\Documents\miccamwatch`**, branche existante conservée | Ancien HEAD `fc17aa7` → livraison `aeb8742` par fast-forward, 81 fichiers actualisés ; vrai `node site/build.mjs` dans ce dossier exporte 0.17.2. Seule note locale bloquante `docs/NEXT_AI_LINUX_MACOS.md` sauvegardée intégralement en preuve privée et stash nommé `mcw-local-handoff-preserved-before-0.17.2-sync-20261013` ; aucun reset/force ni perte du contenu local | Les fichiers livrés sont committés, donc absence de marque « modifié » ne signifie pas absence des corrections. Ne pas supprimer/appliquer aveuglément le stash historique ; comparer avant éventuelle réintégration |
| 2026-10-13 | SITE-TOKENS | **Demande urgente inscrite, non corrigée** : version/source/checksum remplacés par marqueurs visibles | Signalement utilisateur ; templates et substitution/garde du builder relus. Page/artefact actuellement consulté non établi, aucune cause de déploiement inventée | Réparer vrai parcours preview/build/ZIP/publication ; accueil/404/SEO/URLs/curl/provenance sans tokens et checksum script exact |
| 2026-10-13 | SITE-L10N | **Demande urgente inscrite, non implémentée** : site intégral en sept langues MCW | Enum/parser `Language` : en/fr/de/es/ja/zh/ru ; chinois simplifié confirmé par `zh-hans` | Traduction complète, sélection accessible/no-JS, SEO localisé et smokes réels par langue ; valeurs techniques invariantes, SITE-TOKENS prioritaire |
