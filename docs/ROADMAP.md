# Roadmap technique MicCamWatch

Dernière mise à jour : **2026-10-18**. Responsable : lead architecture/développement.
Base publiée : [v0.18.1 stable/latest](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.18.1), source `034d2c1e5cd5a2f06bf40fc69d0b15bed83defcc` ; [CI exacte avant tag](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39027055886) et [release native](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39028452282) vertes ; neuf assets/digests et huit attestations exactes vérifiés. Dépendances ZIP corrigées, SBOM public vérifié. Migrations publiques Windows/Unix, Pages et qualification native Arch0.18.1 réussies. Le18/10, l'utilisateur rapporte sa mise à jour PC vers0.18.1 après Exit manuel du tray ; aucune nouvelle intervention ou observation de cette installation par l'assistant. État des protections non réobservé depuis le relevé du14/10. Recettes externes non publiées.

**P0-DEPS-ZIP : publié 0.18.1.** Les trois avis ZIP émis le17/10 sont traités par self_update1.3.1/ZIP9.0.4 ; audit natif sans ignore et CI quatre OS réussis. Tag et assets0.18.0 conservés immuables.

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
| P0-DEPS-ZIP | **Publié 0.18.1** : self_update1.3.1/ZIP9.0.4, trois avis RustSec ZIP du17/10 traités sans ignore | Moyenne | CI39027055886/release39028452282 quatre OS réussies ; neuf assets/huit attestations et SBOM public vérifiés ; aucun exploit MCW démontré |
| P0-CAM-TRAY | **Publié 0.17.2 et installé sur PC**. Couverture partielle distincte, captures conservées, raisons natives détaillées ; Zoom PID brut valide/report FILETIME zéro, attribution prudente maintenue. Vrai tray PC avec Ktalk actif : menu compact, détails complets, icône rouge ; infobulle résiduelle suivie dans TRAY-WIN | Moyenne | Pas de certification matérielle ; timestamp nul reste Unknown, jamais autorité de kill |
| P0-UNIX-DIAG | **Publié 0.18.0** : captures conservées, avertissement distinct, causes dans Details, lecture inconnue distincte d'une action échouée | Moyenne | CI exacte38863239649 et release38865573731 quatre OS vertes ; limites de mesure et matérielles conservées |
| P0-UNIX-OBS | **Publié 0.18.0, qualifié logiciellement** : inspection macOS passive sans état écrit, lecture globale fixe préautorisée, Cancel natif et disparition du helper au départ du propriétaire | Élevée | Consentement OS Accepter/Refuser avec identifiants humains non exercé ; pas de grant TCC ni preuve de capture, UAC/Polkit inchangés |
| P0-CAM-RESUME | **Reprise réelle qualifiée sur runner zéro périphérique, source non publiée** : garde relancée depuis le vrai tray sans Allow intermédiaire ; cause IPC conservée | Élevée | CI39205263872 : cycle complet passé, nouveau helper authentifié, Block et micro préservés ; 175 tests/Clippy/build passés ; UAC humain et matériel non certifiés |
| P0-WIN-IPC | **Corrigé et scénario natif complet passé en source** : concurrence des clients du pipe admise avec attente bornée | Moyenne | CI39205263872 : vraie libération micro après garde/tray passée ; connexion seule attend3s, pairs conservés, aucune commande rejouée ni mutex d'ownership contourné |
| P1-MAC-MENU | **Publié 0.18.0, AppKit réel qualifié** : quatre profils directs cochés, marque MCW template/badges, Details au-dessus d'Exit, sept langues et `--config` prioritaire | Moyenne | IPC desktop v2 frontend/helper appariés ; captures rendues et vraies fenêtres relues, données de capture simulées explicitement |
| P1-MAC-UX | **Demandé pour la prochaine version, non implémenté** : statut compréhensible, vraie fenêtre de consultation, tray simplifié et À propos concis | Moyenne | Retours utilisateur macOS ; fenêtre AppKit native, diagnostic honnête et revue des actions superflues sans retirer d'accès utile |
| P1-TRAY-PROFILES | **Demandé pour tous les OS** : quatre profils directement sélectionnables dans les menus Windows, Linux et macOS ; macOS livré 0.18.0, Windows/Linux à implémenter et qualifier | Moyenne | Choix directs cochés, logique/persistance existantes, priorité `--config`, souris/clavier et sept langues sur chaque frontend natif |
| SITE-PAGES | **Publié et vérifié sur HTTPS Pages**, source web `9b52e43` : copyright approuvé, racine anglaise et titres longs conservés ; DMG/CLI0.18.1 et sept langues | Moyenne | CI site39127360873 et Cloudflare d1e666cf réussis ;32 surfaces copyright HTTPS et133 scénarios hero antérieurs ; export local EPERM préservé, captures historiques non relabellées |
| SITE-TOKENS | **Corrigé sur Pages, vérifié après déploiement `f1344bf`**. Build Git exécuté depuis racine dépôt, output `site-dist`, version/source/checksum réels ; source HTML brute n'est plus un entrypoint | Faible à Moyenne | Gardes build, source unique des faits, aucun token résiduel |
| SITE-L10N | **Publié et exercé en production** : en/fr/de/es/ja/zh/ru, 227 messages chacun, titres longs et navigation mobile repliables ; DMG localisés conservés | Moyenne | CI site39032505628 ; 133 scénarios HTTPS sept langues, racine sans JS/français/russe et404 root/fr/ru observées ; images171 non relabellées, CSP inchangée |
| SITE-HERO | **Publié et vérifié sur HTTPS**, source `277dd04` : titres longs autorisés à revenir à la ligne et navigation mobile repliable ; texte russe intégral, aucune troncature | Faible | 133 scénarios Chromium sur sept langues, rectangles de texte contre carte, largeurs320–1568, reflow équivalent125/150 % et texte200 % ; captures russes desktop/mobile relues |
| SITE-DEFAULT-EN | **Publié et vérifié sur HTTPS**, source `277dd04` : accueil/404 racine anglais sans redirection de langue ; sept routes et sélecteur conservés | Faible | `/` conservé avec/sans JS et navigateurs français/russe ; métadonnées en_US/canonical `/en/`, choix français, cible installation et404 réelles root/fr/ru exercés |
| SITE-COPYRIGHT | **Publié et vérifié sur HTTPS**, source `9b52e43` : `© 2026 Roman Cuisset` sans tiret sur accueil/404 de toutes les langues ; même nom dans LICENSE | Faible | Quatre tests Node et32 surfaces réelles Pages desktop/mobile ; huit accueils200 et huit véritables404 localisées, lien MIT/public LICENSE, permissions et anglais racine conservés |
| SITE-INSTALL-404 | **Corrigé et exercé en production**. `/install.sh` Pages sert le script exact ; vrai curl serveur ASUS, installation native 0.17.2 puis uninstall possédé sous HOME temporaire | Faible à Moyenne | Maintenir payload/routage et digest ; aucun autostart/tray/protection/root-helper activé |
| P1-RAM | Publié 0.17.0, validé logiciel/natif : bornes et mesures complètes, 15 Mo non atteints, scénario composé Windows ~43 Mo | Moyenne | Maintien des preuves/limites canoniques ; 30 Mo reste une référence |
| P1-RESTORE | Publié 0.17.0, validé logiciel/natif : propriétaire Windows, états opposés/hotplug/partiel simulés, release natif réel sans devices | Élevée | Pas de certification matérielle ou déni exclusif/ASIO |
| P1-HB | Bloqué en qualification : Cellar ARM group-writable 0775, chaîne source Intel z3 défaillante ; aucune publication du tap | Moyenne | Préfixe sûr sans relâcher contrôles et prérequis Homebrew amont fonctionnels |
| P1-AUR | **0.18.1 validé natif** : makepkg/pacman/PipeWire/PTY, install/reinstall/pkgrel-upgrade/remove ; publication AUR bloquée, accès SSH mainteneur non autorisé | Moyenne | CI39029747230/report passed, version attendue lue depuis recette ; publication requiert clé mainteneur AUR autorisée |
| P1-WG | Winget : à faire | Moyenne | Identité MSI et upgrades contrôlés |
| P1-MAC-APP-DMG | **Publié 0.18.1**, DMG Apple Silicon et Intel avec app native, sommes et attestations vérifiées | Moyenne | Finder/doublon/Exit, déplacement, remplacement même build, préférences/historique, autostart possédé enabled/dormant, refus POSIX/retrait et cleanup passés ; ad-hoc seulement, pas d'upgrade inter-version app inventé |
| P1-ARM | Linux ARM64 : à faire | Élevée | ABI/sysroot, installation et tests natifs |
| P1-UPGRADE | **Partiel ; correctif handoff/EOF validé natif en source** : relance Windows sans héritage de handles et sortie coopérative ; refus préalable avec tray actif confirmé séparément dans P1-UPGRADE-ACTIVE | Moyenne | CI39127360852 : appelant capturé terminé avec vrai compagnon vivant ; binaires publics non remplacés par l'assistant, warning ancien CLI mappé toujours ouvert |
| P1-UPGRADE-ACTIVE | **Audité, pas de lease inutilisé établi** : v0.17.2/v0.18.1 partagent le même code d'ownership ; Exit peut résoudre une protection automatique, pas simplement un verrou de frontend | Moyenne | Diagnostics source précisent ressource/requête ; arrêt avant readiness interdit ; migration réelle tray actif à qualifier ; autres OS non déclarés affectés |
| P1-STARTUP-PASSIVE | **Corrigé en source Windows, non publié** : premier état de session connu = baseline passive ; les lacunes Unknown ne créent pas d'arête | Moyenne | Régressions initial Unknown/Locked/Unlocked et transitions ultérieures passées ; protections demandées/restaurations conservées ; aucune action sur le PC |
| P1-UNINSTALL | **Demandé, à faire** : désinstallation rapide par `mcw uninstall` ou point d'entrée équivalent, adapté à la provenance réelle de l'installation | Moyenne | Refus si protection/restauration active, identité/ownership exacts, délégation au gestionnaire MSI/Homebrew/AUR et conservation des données |
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
| TRAY-WIN | **Correctif source en qualification ; PC passé0.18.1 selon l'utilisateur** : identité initiale Shell neutre avant observation, résumé courant borné et UTF-16 sans demi-surrogate | Moyenne | Fixture Shell réelle NIM_ADD/MODIFY, Explorer UIA/hover/recovery sur runner hébergé isolé ; aucune nouvelle observation/interaction avec le tray utilisateur |
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

### Correctifs source du18/10 — qualification en cours

- **TRAY-WIN :** la source courante conserve déjà les comptes Active/Ready sous
  Partial/Error. Le défaut initial restant est l'affirmation d'inactivité fournie
  à `NIM_ADD` avant toute observation ; elle devient une identité produit neutre.
  L'hypothèse d'un cache Explorer expliquant exactement le relevé0.17.2 reste
  non démontrée sur ce PC, qui n'a pas été réobservé. L'infobulle native utilise
  la première ligne courante, marque neutre,127 unités UTF-16 utiles au maximum,
  sans couper une paire de surrogates ; tous les détails restent accessibles.
- **P1-UPGRADE :** une preuve native isolée a montré que `Stdio::null()` pour les
  trois flux ne suffit pas : d'autres handles héritables retiennent encore EOF.
  La relance suit désormais le pattern natif du broker existant :
  `CreateProcessW`, `bInheritHandles=false`, détachement de console, aucun
  handle standard imposé et fermeture des deux handles retournés. Aucun
  changement du protocole d'arrêt coopératif, des refus UAC/busy ou de l'identité.
- **Qualification locale :** le successeur contrôlé reste vivant après la fin
  de l'appelant et EOF stdout/stderr ; stdin reçoit EOF malgré l'entrée originale
  laissée ouverte avec des données. Chemin avec espaces et cyrillique, vraie
  création Windows et sortie coopérative exercés. **CI39127360852 :** le mode
  vrai compagnon a également réussi sur runner hébergé, avec préférences privées
  passives et arrêt authentifié. Aucun tray utilisateur lancé ou arrêté.
- **Qualification Explorer :** CI39127360852 a passé Linux/macOS ARM/Intel et les
  étapes Windows jusqu'au vrai compagnon, puis échoué faute d'infobulle UIA au
  premier survol. Pas de succès visuel revendiqué. Le pilote refuse désormais
  les éléments UIA hors écran, injecte un mouvement natif et conserve coordonnées
  et capture du desktop jetable si l'observation échoue ; source/C# parsées et
  compilées sans exécuter d'entrée sur le PC. Qualification visuelle à terminer.



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

### P1-MAC-APP-DMG — Application macOS prête à ouvrir et image disque

**Demande utilisateur, capture jointe :** ajouter un **`.dmg` pour macOS**,
une **`.app` prête à utiliser**, et démarrer directement dans la **barre de menus**
à l'ouverture de cette application. **Source et deux DMG CI disponibles, non
publiés en release** ; critères de remplacement/retrait en qualification complémentaire.

**Livrable :** image disque contenant `MicCamWatch.app`, installation par Finder
vers Applications et ouverture normale sans compilation ni lancement préalable
d'un terminal. Couvrir Apple Silicon et Intel sur la baseline macOS supportée ;
deux artefacts natifs cohérents suffisent, ne pas promettre un universal non qualifié.
Réutiliser le cœur Rust et le bridge AppKit existants ; aucun Electron, serveur
ou runtime supplémentaire. Garder les archives CLI et leur contrat d'installation.

**Logo autorisé par l'utilisateur :** réutiliser celui du site, source
`site/assets/mark.svg`. L'icône Finder/application et le logo de l'alerte
Details appartiennent à `.app` ; `.dmg` distribue cette application, il ne crée
pas lui-même son identité visuelle. Prévoir les ressources `.icns` et l'icône
de barre de menus adaptées à macOS, lisibles en thèmes clair/sombre et sur Retina.
Ne pas confondre le logo avec le badge d'état actif/partiel/erreur.

**Comportement :** ouvrir `.app` démarre le menu MCW natif immédiatement, sans
fenêtre terminal parasite ; état, diagnostics, commandes et Quit accessibles.
Refuser les doublons de la même instance par identité correcte. L'ouverture
ne vaut ni consentement au démarrage à la connexion, ni activation du mute,
du blocage caméra ou de la terminaison automatique. Quit respecte les intentions
et les restaurations possédées ; aucun release implicite.

**Distribution :** bundle, identifiant/version/icône et exécutables/helpers
cohérents ; `.dmg` versionné dans GitHub Releases, sommes/provenance/SBOM adaptés,
instructions site et documentation à jour. Signature Developer ID et notarisation
seulement avec identité/certificat réels ; sinon annoncer honnêtement les limites
Gatekeeper, sans suppression de quarantaine, bypass TCC/SIP ni certificat fictif.

**Acceptation :** vrais smokes Finder sur macOS ARM et Intel : monter `.dmg`,
copier `.app`, ouvrir, observer la barre de menus, diagnostics/langues et Quit ;
mise à jour/remplacement avec conservation des préférences/historique et de
l'autostart explicitement possédé. Tester bundle déplacé, réouverture/doublon,
permission refusée et retrait ; pas de preuve de capture matérielle déduite du menu.

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

### P1-UPGRADE-ACTIVE — Mettre à jour sans Exit manuel du tray

**Signalement utilisateur du18/10, Windows confirmé :** deux appels à
`mcw update` téléchargent/vérifient la paire0.18.1 puis refusent avec
`native control ownership is busy in another owner/scope; no SDK mutation permitted`.
Après fermeture manuelle du tray par **Exit**, un troisième appel annonce la
mise à jour réussie ; `mcw tray` redémarre ensuite la barre. L'utilisateur
rapporte cette séquence : ne pas la rejouer pour confirmer et ne pas modifier
son installation, ses contrôles ou ses protections pendant l'investigation.

**Défaut à corriger :** un frontend actif de l'installation visée ne doit pas
imposer un Exit manuel lorsqu'une mise à jour sûre est possible. Distinguer ce
refus préalable du défaut EOF après redémarrage déjà corrigé en source.
La présence du tray n'autorise pas à ignorer un vrai propriétaire étranger,
une opération UAC/caméra en cours ou une protection/restauration active.

**Diagnostic et contrat :** identifier le verrou réellement détenu, sa portée,
son propriétaire/génération et l'ordre readiness/arrêt. Examiner notamment les
leases du frontend et des workers inactifs ; ne pas affirmer leur cause avant
lecture et preuve. Réutiliser l'authentification de l'exécutable exact et le
protocole coopératif pour négocier l'état sûr/quiescent puis le remplacement.
Ne pas simplement arrêter le tray avant les gardes : sa sortie ne doit pas
servir à libérer implicitement une protection ou annuler une restauration.
Un Busy réellement dangereux doit rester un refus explicite, sans mutation.

**Audit historique vérifié :** les fichiers pertinents d'ownership, observateurs,
workers tray et updater sont identiques entre les commits publiés
`a58c8b9f3a967589a576879d6d27bfdbc40f9dff` (v0.17.2) et
`034d2c1e5cd5a2f06bf40fc69d0b15bed83defcc` (v0.18.1). Aucun lease global
inutilisé du frontend n'y a été trouvé. La fin du consommateur peut résoudre une
protection micro **automatique**, puis laisser son broker libérer le mutex ;
protection manuelle et Block caméra persistent indépendamment du tray.
Cette possibilité est compatible avec la séquence rapportée, pas sa cause prouvée.
Le nouveau diagnostic source précise caméra/micro et requête/ressource ; l'ordre
des gardes reste inchangé. Le CLI **qui invoque** update exécute l'ancien
transactionnel : un binaire téléchargé ne corrige pas rétroactivement ce client.
Version invocatrice, mutex et état exact lors des refus historiques restent inconnus.


**Acceptation :** sur Windows jetable, exercer le vrai `mcw update` avec le vrai
tray de la même installation déjà actif : vérification du paquet, négociation et
arrêt coopératif, swap de la paire, redémarrage d'un seul tray à la bonne version
et fin de l'appelant avec EOF, **sans Exit manuel**. Préférences, historique,
autostart et intentions de protection préservés ; erreurs/busy/UAC/protection
active, propriétaire étranger, PID réutilisé et rollback ne doivent ni tuer un
processus, ni muter le SDK, ni contourner les contrôles. Les anciens clients/
artefacts ne sont pas réécrits : prévoir une migration explicite si nécessaire.

**Périmètre et distinction :** macOS/Linux sont à auditer, pas déclarés affectés ;
respecter leurs contraintes de provenance et d'exécutable actif au lieu de copier
une fermeture forcée Windows. L'avertissement séparé d'accès refusé au backup
`.mcw-update-*` sous `%USERPROFILE%\.cargo\bin` reste suivi dans P1-UPGRADE :
aucun nettoyage automatique sur le PC ni récupération d'un fichier en quarantaine.

### P0-CAM-RESUME — Reprendre la garde caméra sans Autoriser/Rebloquer

**Relevé utilisateur du18/10, sans nouvelle interrogation de son PC :**
`0 active (0 Mic, 0 Cam); 0 ready`, protection micro demandée avec garde active,
sourdine SDK et un endpoint annonçant la capacité hardware mute ; blocage global
caméra demandé, `present:1 blocked:1 pending:0 absent:0 unknown:0`, mais garde
inactive et avertissement que les futures arrivées ne sont pas protégées.
Le contournement rapporté est **Autoriser puis Rebloquer**.

**Interprétation bornée :** zéro capture observée n'est pas un refus
d'autorisation de capture. La sourdine endpoint n'est pas une permission de
capture, et la capacité hardware mute n'est pas une preuve de commutateur
physique actionné. Le statut d'erreur ne doit pas être masqué : l'intention
caméra et le périphérique bloqué ne prouvent pas une garde vivante.
Ce relevé ne fixe pas rétrospectivement l'état des protections lors des trois
tentatives d'update précédentes.

**Correctif attendu :** diagnostiquer la disparition/non-reprise du helper et
permettre de réarmer la garde de l'intention Block existante, sans transition
Allow, sans abandon de l'ownership ni restauration d'un périphérique.
Réutiliser le protocole authentifié et les générations ; demander explicitement
le consentement UAC lorsque nécessaire, sans élévation silencieuse au lancement.
Un refus UAC, une portée étrangère ou un helper incompatible reste un état
non protégé explicite, pas un succès. Qualifier la reprise, l'annulation,
les nouvelles arrivées et les intentions concurrentes en environnement isolé ;
ne pas exercer les contrôles ou une capture sur le PC utilisateur.

**Source implémentée, pas encore publiée :** le menu conserve Autoriser et ajoute
une action de reprise explicite seulement pour Block demandé/garde indisponible.
Sous le request lock, la reprise refuse toute intention Allow plus récente ou
contradictoire avant écriture/élévation ; elle réutilise Block sans transition
Allow et conserve la restauration automatique du tray. L'erreur native de la
requête Status est désormais conservée : l'ancien code effaçait tout échec IPC
et le décrivait comme un helper arrêté. La cause historique sur le PC n'est donc
pas établie. 173 tests ordinaires Windows et Clippy/build passés ; scénario réel
tray → perte du helper possédé → reprise sans Allow ajouté au runner isolé,
inventaire natif obligatoirement nul. Consentement UAC humain, appareil bloqué
physique et futures connexions physiques restent hors preuve de ce runner.


### P1-STARTUP-PASSIVE — Démarrage passif sûr

**Choix utilisateur confirmé le18/10 :** observer sans nouveau blocage et sans
prendre de verrou technique inutile ; conserver les protections déjà demandées
ainsi que l'ownership nécessaire à leur restauration. Cela n'autorise ni remise
à zéro des intentions persistées ni ouverture automatique de caméra/micro.

**Hypothèse à vérifier, pas un diagnostic :** l'utilisateur suspecte que les
« verrous » au démarrage cachent les caméras. Le mutex d'ownership et le blocage
SDK sont distincts ; les constructeurs Windows lus sont observateurs, sans
acquisition de lease global. Vérifier les chemins de reprise existants et la
première observation de verrouillage de session : un premier état connu ne doit
pas être pris pour une nouvelle transition déclenchant un blocage automatique.
Les transitions ultérieures configurées et les protections antérieures restent
respectées. Le cas P0-CAM-RESUME est une garde absente avec blocage demandé,
pas une raison de supprimer ce blocage.

**Changement source Windows :** un état de politique distinct retient le dernier
état de session connu. Le premier état valide initialise cette baseline sans
requête de contrôle ; Unknown n'invente ni verrouillage ni déverrouillage.
Les transitions ultérieures et leur restauration continuent via les workers
existants. Tests des trois états initiaux, lacunes et transitions passés ;
aucune remise à zéro d'intention persistante ni ouverture de périphérique.


### P1-UNINSTALL — Désinstallation rapide depuis MCW

**Demande utilisateur :** pouvoir retirer MCW rapidement avec **`mcw uninstall`**
ou une commande native équivalente, sans devoir retrouver manuellement le script
ou la paire d'exécutables. État : demandé, **non implémenté**.

**Contrat :** identifier l'installation exacte de l'exécutable courant et sa
provenance : installateur Unix géré, Windows portable/Cargo, MSI, gestionnaire
Homebrew/AUR, future `.app` macOS. Réutiliser les contrats de receipt/ownership
et de lifecycle existants ; ne jamais supprimer un autre MCW trouvé sur PATH.
Si un gestionnaire possède l'installation, utiliser sa procédure appropriée
ou afficher la commande exacte lorsque sa délégation n'est pas sûre/autorisée,
sans inventer un succès. Une inscription autostart externe reste intouchable.

**Sécurité :** ne pas libérer silencieusement le micro, autoriser les caméras,
annuler une restauration ou interrompre une opération UAC pour désinstaller.
Protection demandée/active, dettes pending/unknown ou helper privilégié installé
= refus explicite avec la procédure de restauration/retrait appropriée.
Après disponibilité établie, arrêter uniquement le frontend possédé de cette
installation, retirer seulement ses payloads/receipts/blocs PATH/autostart
inchangés, et gérer le retrait de l'exécutable Windows encore mappé sans kill
forcé ni contournement antivirus/ACL/quarantaine.

**Données :** conserver par défaut politiques, préférences et historique.
Une suppression de données, si proposée, exige une option et un consentement
explicites ; aucune suppression récursive d'un dossier utilisateur arbitraire.

**Acceptation :** vrai parcours natif de chaque provenance prise en charge,
dans un environnement jetable : succès/retrait vérifiable, no-op déjà retiré
lorsque représentable, cible étrangère/modifiée/liée refusée, protection active
préservée et gestionnaire respecté. Mettre à jour aide CLI, site dans les sept
langues, documentation et procédures de retrait. Ne pas essayer de désinstaller
le MCW réellement protégé du PC utilisateur pour ce smoke.

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
et de la somme du script. Défaut utilisateur établi, **désormais corrigé en production**.
Les smokes antérieurs ne le réfutent pas.

**Cause établie :** configuration Git Cloudflare fournie par l'utilisateur :
racine `site`, build et sortie vides, donc publication des templates bruts.
Après réglage racine dépôt vide/build `node site/build.mjs`/sortie `site-dist`,
déploiement `f1344bf` réussi et vrai HTTPS sans marqueurs, version 0.17.2.
Cutover source : templates sous `site/templates/`, texte dans `site/locales/`,
export seul destiné aux visiteurs ; aucun remplacement JS qui masquerait le défaut.

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

### SITE-INSTALL-404 — Commande curl Pages en échec HTTP 404

**Signalement utilisateur historique sur le serveur ASUS ; corrigé après réglage Git :**

```console
serveur@serveur-asus:~/test$ curl --proto '=https' --tlsv1.2 -fsSL https://miccamwatch.pages.dev/install.sh | sh
curl: (22) The requested URL returned error: 404
```

Le signalement initial est établi ; aucune requête de simple confirmation.
Après correction/déploiement Git réussi, cette URL sert réellement le script
text/plain et le digest exact. Vrai curl HTTPS depuis ASUS : install/version
0.17.2/uninstall possédé réussis dans HOME/XDG temporaires, sans activation.

**Correction urgente attendue :** identifier le contenu effectivement publié
et le routage de `/install.sh`, garantir que le script revu est inclus à la
racine du payload Pages et que la commande affichée cible une URL réellement
servie. Examiner le build/export, le dossier ou ZIP déposé et les règles de
routes sans supposer quel élément est fautif. Ne pas contourner par une page
HTML 200, un miroir de binaire ou une désactivation de la vérification TLS.

**Acceptation :** après correction, cette URL HTTPS exacte retourne 200 et le
script shell attendu, pas la 404 HTML ; SHA-256 concordant avec la source
immuable et les faits de release. Exécuter le parcours annoncé sur Linux dans
un HOME/préfixe temporaire, observer la version native installée et le retrait
possédé sans activer autostart/tray/protection ni helper root. Un échec curl
doit rester explicite : le succès éventuel de `sh` recevant une entrée vide
ne doit jamais être présenté comme un succès d'installation.


### SITE-L10N — Site complet dans toutes les langues MCW

**Demande utilisateur urgente :** couvrir exactement les langues déclarées par
`src/i18n.rs::Language` : **anglais `en`, français `fr`, allemand `de`,
espagnol `es`, japonais `ja`, chinois simplifié `zh`, russe `ru`**.
État actuel : **sept langues intégralement implémentées, publiées et vérifiées sur HTTPS**, source web `07eb173`.

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

### SITE-HERO — Titre long masqué par la carte sur desktop

**Signalement utilisateur, capture 1568×658 :** sur la page russe, le titre
« Ваша осведомлённость » dépasse sa colonne ; la fin du mot passe derrière la
carte de capture Windows et devient illisible. Dimension de l'image connue,
viewport CSS/zoom exacts non établis. Défaut visuel établi par la capture.
**Publié et vérifié sur HTTPS le17/10, source web `277dd04`.**

**Correction appliquée :** les titres partagés autorisent la césure selon la
langue et le retour à la ligne de secours (`hyphens: auto`, `overflow-wrap:
anywhere`). Les colonnes conservent leur `min-width: 0` existant ; la navigation
mobile peut passer sur plusieurs lignes pour les libellés longs. Aucun texte
masqué, raccourci ou tronqué ; aucune réduction artificielle de la traduction.

**Acceptation :** titre russe intégralement lisible, aucune collision entre
texte et carte ; même qualité pour les sept langues en desktop/tablette/mobile,
aux largeurs voisines des breakpoints et avec zoom 100/125/150 %. Contrôler le
chevauchement réel des éléments, pas seulement `scrollWidth > innerWidth`.
Les smokes précédents restent datés et ne prouvent pas cette absence de collision.

**Qualification privée du17/10 :** export réel sous Wrangler Pages4.149.0,
quatre tests de localisation réussis et 133 scénarios Chromium sur les sept
langues : 56 largeurs normales320/390/800/801/900/1024/1280/1568, 21 cas avec
texte agrandi200 %, 56 reflows équivalents aux zooms125/150 % pour les surfaces
1568/1280/1024/390. La mesure porte sur les rectangles des fragments de texte,
leur inclusion dans la colonne et leur intersection avec la carte, pas seulement
la largeur globale. Captures russes desktop et mobile relues ; aucune collision,
troncature ou largeur globale excédentaire dans les cas normaux/de reflow.
Les zooms sont émulés par viewport CSS/DPR, pas par le menu du navigateur.

**Qualification HTTPS du17/10 :** les mêmes133 scénarios ont été exercés sur
le déploiement réel ; aucune collision de fragments avec la carte ni troncature.
Captures russes desktop/mobile relues. CI site
[39032505628](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39032505628)
réussie et check Cloudflare Pages du commit `277dd04` réussi
(déploiement `5ff8f167-00e5-40ac-ae51-e707eebf683d`).

### SITE-DEFAULT-EN — Anglais à la racine sans préfixe

**Demande utilisateur :** ouvrir **`https://miccamwatch.pages.dev/`** sans
`/en/`, `/fr/` ou autre préfixe doit afficher le site **en anglais**, pas en
français. **Publié et vérifié sur HTTPS le17/10, source `277dd04`.**
La présence de `/en/` seule ne satisfait pas cette demande.

**Correction appliquée :** le générateur choisit le catalogue anglais pour
l'accueil et la404 racine, sans redirection ni détection automatique de langue.
Les sept routes, le choix manuel et les404 localisées restent présents.
`lang="en"`/`og:locale=en_US`, canonical `/en/` et x-default `/` sont cohérents
avec le contenu anglais ; le catalogue français reste le maître de validation
des traductions, distinct de la langue par défaut des visiteurs.

**Acceptation :** vrai HTTPS `/` anglais, URL racine conservée, avec et sans
JavaScript, même si le navigateur est en français/russe. Liens d'installation,
sélecteur et sept routes fonctionnels ; canonical/hreflang/x-default/sitemap
cohérents, sans contradiction entre langue déclarée et contenu. Documentation,
instructions de publication et nouvelle archive reflètent le défaut anglais.

**Qualification HTTPS du17/10 :** racine200 anglaise sans changement d'URL,
avec et sans JavaScript et navigateur français/russe ; vrai choix de langue vers
`/fr/`, cible d'installation visible après défilement, `lang`/og:locale/canonical/
hreflang/x-default cohérents. Routes inconnues :404 anglaise à la racine,
française sous `/fr/` et russe sous `/ru/`. Nouvelle archive complète produite
en privé et artefact `miccamwatch-pages` de la CI39032505628 récupéré :
accueil/404 root anglais, CSS qualifiée identique hors CRLF Windows/LF Linux,
script install.sh SHA b734 exact. Aucun nouvel installateur binaire ou tag.

### Résultat web du 2026-10-14

- **Source :** layout partagé `site/templates/`, sept catalogues `site/locales/` de 223 messages chacun ; `localization.mjs` refuse clé manquante/extra/vide, markup/code/URL/token technique modifié ou HTML déséquilibré. Aucun fallback silencieux. Quatre régressions Node passent, dont le vrai défaut initial des clés metadata avec `:` ; CI statique ajoutée.
- **Vrai runtime Pages :** chaque langue desktop 1366×900 et mobile 390×844, sans overflow, images chargées ; navigation de langue directe, onglets OS ArrowRight/End, feedback d'échec de copie réellement localisé avec permission native refusée et texte sélectionné. Aucun clipboard utilisateur écrit. Sans JS, sept langues navigables et toutes les sections OS visibles ; commandes `--lang` cohérentes.
- **HTTP :** accueil par langue 200, vraie route profonde inconnue 404 dans la langue du préfixe, pages/commandes/liens sans marqueurs. Canonical/hreflang/sitemap et cartes sociales par langue, CJK/cyrillique vus réellement ; typo mobile JP/RU ajustée après observation. Screenshots Windows 0.17.1 inchangés et explicitement historiques.
- **Origine :** canonical du premier déploiement observé sur le hostname preview `827b9da7.miccamwatch.pages.dev`. Source corrigée : `site/origin.json` = vraie origine production `https://miccamwatch.pages.dev`, override explicite `MCW_SITE_URL` ; ne plus prendre `CF_PAGES_URL` éphémère comme canonical. Export sans override avec cette vraie URL preview en environnement garde bien l'origine production.
- **Production française réparée :** check Cloudflare Git `f1344bf` success ; HTTPS placeholders absents, script servi. Vrai curl `/install.sh` depuis serveur ASUS, SHA `b7348c9a6f37a3d4f89faa2b767fd4880162d90c8f5d7c151185e0823384d1bb`, version native 0.17.2 et retrait possédé réussis, sans root/helper/tray/autostart/contrôle.
- **ZIP multilingue complet :** `Téléchargements/miccamwatch-pages-0.17.2-multilingual.zip`, 490 598 octets, 34 fichiers, index et install.sh à la racine ; SHA-256 `1747056b969ea5ca017895743af6d6b05eeb6f925c391af4eba8b6caa720bf65`. Pas de catalogues/templates/manifestes de preuve privés dans le payload.
- **Limite Windows explicite :** remplacement du `site-dist/` existant refusé par EPERM au rename. Cause de verrou/permissions non établie, ancien export préservé, pas de suppression forcée/ACL/exclusion AV. Sources exactes exportées avec succès dans sorties neuves ; ne pas confondre le vieux dossier local et la nouvelle archive complète.
- Preuves privées `mcw-site-l10n-yj3keepa/` : exports, UI/no-JS/routes/typographie, curl production, livraison et `public-proof.json`. Publication Git `07eb1734dd177a5319969da5645983a6b94097d5` Cloudflare success et [CI site38530338888](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38530338888) réussie ; chaque langue HTTPS vérifiée, canonical production stable, vraie 404 localisée et commande same-origin, aucun token ; script200/textplain/51249/SHA exact.
- Hébergement : Cloudflare injecte `static.cloudflareinsights.com/beacon.min.js`, **bloqué par notre CSP** en production observée. Aucun tracker ajouté par le code ni whitelist ; ne pas élargir CSP pour faire taire ce warning. Pour supprimer aussi l'injection, désactiver Web Analytics dans le projet Cloudflare via accès compte autorisé ; aucun changement de ce réglage ou de l'identité de compte inventé.



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

## P0-UNIX-DIAG — Observation partielle et faux échec d'action

**Relevé utilisateur macOS 0.17.2 reçu :** `Telemetry degraded: corespeechd:
MIC ACTIVE`, puis `Action failed: Camera restriction status unknown:
Current-user profiles contain no visible camera restriction; device/other-user
scope is not readable, so effective status is unknown.` Le verrouillage est
également inconnu, les contrôles automatiques indisponibles. Ce relevé est une
observation utilisateur, pas un nouveau smoke sur son Mac.

**Causes établies dans la source :** `coremediaio_video` reste Degraded car
CoreMediaIO donne l'état de fonctionnement du périphérique, sans identité client
ni preuve de flux. Une erreur de lecture de `camera_state` était placée dans le
feedback d'action et rendait l'icône Error, même sans action caméra. La vue compacte
réduisait aussi les détails de l'observation à un libellé générique. Le probe de
verrouillage public macOS retourne volontairement Unknown : les clés publiques
Quartz établissent le contexte console, pas un état de verrouillage actuel.

**Correction source :** activité active/ready au premier plan, observation
partielle orange distincte d'une indisponibilité réelle, collecteur et raison
complets dans Details. Lecture caméra inconnue séparée du feedback d'action ;
elle ne devient ni Allowed ni Blocked. Garder l'approbation manuelle macOS
accessible : `toggle_camera` sait préparer un profil possédé même si l'absence
de restrictions globales n'est pas établie. Une lecture saine suivante récupère
son propre état sans acquitter une action échouée. Aucun changement de permission,
profil installé, mute, contrôle matériel, autostart ou application capturante.

**Point d'arrêt demandé le 2026-10-14 :** source
`0522733c3254131f357f4c9b878a6ff376e14ea1` committée et poussée sur `main`,
sans tag ni nouvelle release. Local Windows : `cargo fmt --all`, Clippy
`--all-targets --features windows-tray --locked -- -D warnings`, **167 tests
réussis**, puis build release réussi. Le contrôle de syntaxe Python de la
fixture macOS a réussi ; il ne prouve pas son exécution AppKit.
Ces vérifications Windows ne compilent/exercent pas le frontend Unix conditionnel.

**Reprise :** la [CI source exacte 38613211447](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38613211447)
a **échoué**, elle n'est plus en cours. Apple Silicon a réussi, menus et Details
AppKit réellement affichés et relus. Linux a échoué sur une ancienne assertion
de rendu Error, Windows sur quatre nouveaux avis rustls, Intel sur le candidat
TUI qui ne quittait pas après Q dans une rafale. Assertion incidente supprimée,
rustls 0.23.46 et backend tty crossterm level-triggered intégrés, avec régression
redimensionnement puis Q sans drain intermédiaire. La cause événementielle Intel
est cohérente avec Mio [INFERENCE], non tracée. Échantillonnage au-delà de 50 ms
(écart max 420,5 ms) : aucune qualification ressources déduite de ce run.

**Lot repris :** source **0.18.0 non publiée**, car nouvelle commande macOS et
rupture IPC desktop v2. Diagnostic administrateur ciblé, profils directs, icône
MCW et packaging app/dmg intégrés ; CI38716101572 quatre OS entièrement verte.
Le site, les recettes publiques et la paire PC restent **0.17.2**. Aucun tag,
restart utilisateur, mute/unmute, Block/Allow, profil, autostart, permission ou
application capturante modifié sur le PC.

## P0-UNIX-OBS — Causes réelles, permissions et autorisations utiles

**Priorité demandée après le diagnostic :** réparer ce qui empêche réellement
l'observation, pas seulement remplacer « Telemetry degraded ». Examiner les
erreurs natives CoreAudio/CoreMediaIO, identité/birth/exécutable, visibilité des
profils macOS et collecteurs PipeWire/V4L2/proc Linux. Conserver les captures
établies pendant une lacune et distinguer absence d'information d'absence d'accès.

**Diagnostic actionnable demandé :** pour chaque ressource, nommer le collecteur,
ce qui est observé, ce qui manque, la cause native et le remède réellement utile.
Ne dire « droits manquants » qu'avec preuve de permission/périmètre insuffisant.
Ici, la requête `profiles` non-root n'emploie pas `-all` et ne peut établir un
inventaire global ; les détails indiquent désormais qu'une lecture globale
nécessite une autorisation administrateur. Ce n'est pas une autorisation TCC.

**Parcours implémenté en source :** `mcw camera inspect --json` n'élève rien et
n'écrit ni reçu ni état utilisateur. Si le périmètre est incomplet,
`mcw camera inspect --admin` ou l'action explicite du menu propose la confirmation
MCW puis l'autorisation macOS. Seule une commande système fixe lit les métadonnées
globales ; aucun chemin/identité/commande ne vient de la requête. Capture bornée
à 1 Mio/15 s avant matérialisation AppleScript, timeout autorisation 110 s et
requête native 120 s, après une éventuelle lecture passive de 15 s. Pas de
garantie de 120 s de bout en bout. Annulation/refus restent inconnus/incomplets.
Les résultats horodatés ne changent ni les contrôles ni l'ownership des profils.
Le worker borné préserve le polling, la session utilisateur et les autres erreurs.

Windows UAC et Linux Polkit réutilisent leurs parcours ciblés existants ; aucun
besoin de nouveaux droits d'observation n'a été établi ici. CoreMediaIO sans
PID/flux et le verrouillage public inconnu sont des limites API, pas des défauts
réparables par admin. Aucun contournement TCC/SIP/quarantaine, aucune élévation
globale, aucune prétention d'observer les clients caméra après élévation.

**Qualification intégrée réussie :** [CI38716101572](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38716101572),
source `40ed84d81f8cbcb978684b045385f68d7d8400da`, Windows/Linux/macOS ARM/Intel.
Sur les deux Mac : vraie CLI passive sort 2 avec scope incomplet et proposition
admin, sans état créé ; parser/bornes/ownership, Cancel AppKit avant demande OS,
fermeture du propriétaire et commande globale fixe sous sudo préautorisé réussis.
La lecture globale retourne `allowed` pour les seuls profils, sans ownership
adopté ; ce n'est ni TCC ni disponibilité matérielle. Le PATH hérité de `env -i`
restait tainted dans Perl `-T` : affectation littérale interne corrigée, protection
`-T` conservée, avant/après reproduit et lecture native confirmée.

Les clics Accepter/Refuser du dialogue OS avec identifiants humains restent non
exercés. La cohérence Granted/Cancelled/Denied est testée à la frontière Rust ;
aucun clic ou grant OS inventé. Le relevé du Mac utilisateur reste indépendant.

## P1-MAC-MENU — Retours AppKit à traiter

**État :** critères visuels, choix directs et navigation clavier réelle qualifiés.
CI38859102456 `b84976f` : Home/Down/Right/Return sélectionne directement Meeting
dans le vrai tracking AppKit, sans événement global ni action de contrôle.
Menus longs 399 pt, détails complets et Copy all natif ;
cas quatre sélections, priorité config explicite, partial→recovery et badges
clair/sombre 1×/2×. Le badge macOS template respecte le contraste système ; ne
pas promettre une couleur orange sur cette icône monochrome.

- **Profil :** sous-menu avec choix direct Balanced/Private/Meeting/Development,
  état sélectionné visible ; ne plus imposer le cycle successif. Un `--config`
  explicite reste prioritaire sur le profil de préférence.
  La sélection directe n'est pas réservée à macOS : la parité Windows/Linux est
  suivie séparément dans P1-TRAY-PROFILES ci-dessous.
- **Icône :** identité MCW reconnaissable avec le logo autorisé du site ; expliquer
  les badges d'état, sans remplacer une activité par un symbole générique.
- **Ordre :** placer Details au-dessus de Exit ; vérifier navigation clavier,
  diagnostic complet, lisibilité des sept langues et menu réellement affiché.
- **Distribution :** `.app`/`.dmg`, logo Finder et barre de menus relèvent de
  P1-MAC-APP-DMG ; DMG Apple Silicon/Intel publiés depuis 0.18.0, version courante indiquée dans la base publiée.

## P1-MAC-UX — Prochaine version : statut, fenêtre, menu et À propos

**Origine :** retour de l'ami de l'utilisateur sur macOS : le statut reste peu
compréhensible, aucune fenêtre de programme ne permet de consulter les
informations, certains éléments du tray semblent superflus et une courte
présentation dans « À propos… » manque.
**État :** demandé pour la prochaine version ; non implémenté, sans numéro de
version future figé. Les validations du menu actuel ne clôturent pas ces retours.

1. **Clarifier le statut.** Présenter un résumé lisible du micro et de la caméra,
   en distinguant activité observée, protection demandée/effective et couverture
   incomplète. Expliquer simplement les états inconnus/dégradés et leurs causes,
   sans masquer une limite API ni faire croire qu'un droit admin suffit toujours.
2. **Ajouter une vraie fenêtre de consultation.** Fournir une fenêtre AppKit
   native accessible depuis le tray ; la réouverture de l'app doit permettre
   d'afficher la fenêtre existante sans doublon. Montrer l'état micro/caméra,
   les accès connus et les diagnostics utiles, sans imposer le Terminal ni se
   limiter à une alerte « Details ». Conserver un fonctionnement discret en
   arrière-plan ; fermer la fenêtre ne doit pas implicitement quitter le tray
   ou modifier les contrôles.
3. **Simplifier le menu tray.** Examiner les entrées réellement utiles, retirer
   les doublons et regrouper/déplacer les options avancées dans la fenêtre si
   pertinent. Les éléments précis à retirer restent à décider après cette revue :
   ne pas supprimer arbitrairement une commande ou l'accès aux diagnostics.
   Conserver la sélection directe des profils prévue par P1-TRAY-PROFILES.
4. **Ajouter « À propos… ».** Afficher la version et une présentation courte,
   par exemple : « MicCamWatch est une application de surveillance des accès
   au microphone et à la caméra. Elle affiche les informations rendues visibles
   par le système, sans enregistrer de son ni d'image. » Ne pas promettre une
   attribution ou un blocage que macOS ne permet pas.

**Validation :** vrais statut/fenêtre/menu/À propos sous macOS, compréhension des
états actif/inactif/inconnu/dégradé, ouverture/réouverture/fermeture sans doublon,
navigation clavier et lisibilité des sept langues. Aucun droit, autostart ou
contrôle de confidentialité ne doit être activé implicitement par ces surfaces.

## P1-TRAY-PROFILES — Sélection directe des profils sur tous les OS

**Demande utilisateur :** proposer les quatre profils directement dans les menus
natifs Windows, Linux et macOS, pas uniquement dans le menu AppKit.
**État :** macOS livré et qualifié depuis 0.18.0 ; Windows et Linux restent à
implémenter puis à exercer réellement. Ce lot n'est pas annoncé comme déjà livré
sur ces deux plateformes.

- Présenter Balanced, Private, Meeting et Development comme quatre choix directs,
  avec le profil actif clairement coché ; ne pas imposer de clics successifs pour
  faire défiler les profils.
- Réutiliser la logique de profil et la persistance existantes ; conserver la
  priorité d'un `--config` explicite et rendre visible un choix désactivé/non
  applicable plutôt qu'une fausse sélection.
- Vérifier les vrais menus natifs Windows, Linux et macOS : sélection à la souris
  et au clavier, changement du choix coché, conservation après redémarrage,
  configuration explicite et lisibilité des sept langues. Une preuve AppKit
  seule ne valide pas Windows ou Linux.

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
| 2026-10-13 | SITE-INSTALL-404 | **Incident urgent inscrit, non corrigé** : vrai curl utilisateur Pages retourne 404 | Commande depuis `serveur@serveur-asus:~/test` vers `https://miccamwatch.pages.dev/install.sh`, erreur curl 22/HTTP 404 ; pas de nouvelle requête de confirmation | Qualifier payload/routage Pages, servir vrai script vérifié à cette URL, puis smoke HTTPS install/version/uninstall isolé ; les preuves du DNS ASUS ne couvrent pas Pages |
| 2026-10-14 | P1-MAC-APP-DMG | **Demande inscrite, non implémentée** : `.dmg`, `.app` prête à ouvrir et lancement direct dans la barre de menus | Capture utilisateur en russe : image disque macOS, application préparée, démarrage dans le tray/menu ; trois attentes distinctes conservées | Packaging natif ARM/Intel, Finder/menu/Quit/upgrade réels ; aucune permission, protection ou activation autostart implicite |
| 2026-10-14 | TRAY-WIN / garde caméra | **Nouvelle croix grise diagnostiquée puis garde réactivée avec accord UAC explicite** | Vrai tray 0.17.2 : micro demandé/garde active/muted, caméra Block possédée mais helper élevé inactif ; branche `refreshed_status` traite ce trou de protection comme Error. Après choix « Réactiver avec UAC », `mcw camera block` rapporte garde active ; même PID tray, panneau natif quitte Status error, micro reste demandé/muted, caméra blocked/owned1/pending0/unknown0 | Ce n'était pas un collector dégradé ni une caméra absente inventée. Origine de l'arrêt du helper non établie ; persistance après session/reboot à diagnostiquer sans élévation implicite ni masquer l'avertissement. Aucune commande Allow, unmute ou restart tray |
| 2026-10-14 | SITE-TOKENS / SITE-INSTALL-404 | **Cause de publication établie par configuration Git Cloudflare fournie** | Racine `site`, build et sortie vides : templates bruts publiés, pas d'installateur généré. Réglage attendu : racine dépôt vide, `node site/build.mjs`, sortie `site-dist` ; utilisateur choisit de régler les trois champs | Vérifier vrai déploiement Git et HTTPS après réglage, pas déclarer réparé avant preuve. Relay navigateur non connecté, aucun token CF en environnement ; pas de modification de compte/cloudflare inventée |
| 2026-10-14 | P1-UNINSTALL | **Demande inscrite, non implémentée** : `mcw uninstall` ou commande équivalente pour retrait rapide | Point d'entrée unique souhaité, sans confusion MSI/portable/installer/package manager ni perte des données | Ownership exact, gardes/refus/restauration et frontend possédés, vrai smoke jetable par provenance ; aucune désinstallation du PC actuel |
| 2026-10-14 | SITE-TOKENS / SITE-INSTALL-404 | **Réparés en production et exercés**, déploiement Git `f1344bf` | Configuration corrigée, vrai HTTPS version/source sans tokens, script exact51249/SHA b734 ; curl Pages natif ASUS install/version172/uninstall isolé réussi | Maintenir root/build/output corrects, origine canonical stable ; aucun privilège/app/privacy implicite |
| 2026-10-14 | SITE-L10N | **Implémenté et qualifié ; publication en cours** | Sept catalogues223/templates/JS feedback/SEO/404/social ; quatre tests Node, vrai desktop/mobile/clavier/permission clipboard refusée/no-JS et 404 localisées ; ZIP490598/34 fichiers/SHA1747056b | Vérifier nouveau déploiement public ; EPERM ancien export Windows préservé, frais exports passent sans bypass |
| 2026-10-14 | SITE-L10N / SITE-PAGES | **Publié HTTPS et smoke production réussi**, source `07eb173` | Cloudflare deploy4653ef80 success ; [CI38530338888](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38530338888) success ; sept langues/404/canonical stable/images/commandes/valeurs réelles, script20051249SHA b734. Locale source/code ne change pas les binaires172 ni gardes PC | Windows export préexistant EPERM préservé, ZIP complet livré ; beacon injecté par Cloudflare bloqué CSP, retirer l'injection via réglage Web Analytics plutôt que permissivité CSP |
| 2026-10-14 | SITE-HERO | **Petit défaut UI inscrit, non corrigé** : titre russe masqué par carte desktop | Capture utilisateur1568×658 : fin de « осведомлённость » derrière la carte ; viewport CSS/zoom inconnus, aucune nouvelle mesure de confirmation | Grille/typo/retours à la ligne pour textes longs ; vrai contrôle collision et lisibilité toutes langues/largeurs/zoom, sans troncature |
| 2026-10-14 | SITE-DEFAULT-EN | **Demande inscrite, non appliquée** : anglais sur URL racine Pages sans préfixe | Utilisateur demande `https://miccamwatch.pages.dev/` anglais, pas seulement `/en/` ; actuel défaut français identifié dans contrat export | Accueil/404 root en anglais, lang/SEO/nav cohérents ; conserver toutes langues et URL racine, pas de choix automatique imposé |
| 2026-10-14 | P0-UNIX-DIAG / P0-UNIX-OBS / P1-MAC-MENU — arrêt demandé | **Source `0522733` poussée, qualification Unix non terminée ; arrêt pour aujourd'hui** | Local Windows : fmt/Clippy/167 tests/build release réussis ; cause macOS établie par relevé utilisateur et source. CI exacte [38613211447](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/38613211447) en cours au dernier relevé, aucun succès natif revendiqué. Aucun nouveau tag/release, aucune installation ou protection changée | À la reprise : résultat/preuves CI Linux/macOS, puis causes réelles d'observation et élévation ciblée explicite utile. Sous-menu de profils, identité MCW, Details au-dessus d'Exit et logo site pour app/dmg inscrits, non livrés |
| 2026-10-14 | Reprise P0-UNIX-DIAG / OBS / MAC-MENU / APP-DMG | **Source 0.18.0 intégrée, qualification native en cours, non publiée** | Windows local : fmt/Clippy, 167 tests, build release et vraie CLI version/aide réussis ; cargo audit sans avis signalé. Ancienne CI38613211447 examinée : ARM AppKit réussi, échecs Linux/assertion, Windows/rustls et Intel/Q réparés sans relâcher gates/budgets. Inspection lecture seule explicite, menu IPCv2 et packaging natif intégrés ; six fixtures Python et YAML CI/release syntaxiquement valides | CI native exacte et preuves ; ne pas assimiler sudo runner à consentement OS interactif, ni exit77 AppKit à un lifecycle PASS. Paire PC et stable 0.17.2 préservées |
| 2026-10-15 | Qualification 0.18.0 `3c2c9ae` | **CI38706382960 échouée sur isolation du harness Mac ; Windows/Linux réussis**, aucun tag | Les deux Mac compilent/testent/construisent réellement app+DMG et passent ressources/TUI/tray/stop, puis l'invariant préservant le HOME login détecte un dossier MCW. Cause source établie : nouveau test Rust de forwarding appelle `make_view`→`autostart::state`→`Store::open`, qui crée l'état hors sandbox ; test incident supprimé, invariant conservé. Vraie capture menu ARM relue : ordre/icône/profil présents, libellé admin trop long raccourci pour garder « lecture seule » visible | Requalification exacte requise ; smokes Mac indépendants exécutés même après échec d'un autre smoke, sans rendre la CI verte ni masquer une erreur. Aucun état utilisateur réel effacé |
| 2026-10-15 | Qualification 0.18.0 `6e9f0de` | **CI38709115147 : Windows/Linux et smoke principal Mac réussis ; deux smokes ciblés Mac encore en échec** | ARM/Intel : vrai DMG monté/copié/scellé, app Finder prête, doublon préservé, clic natif Exit et absence autostart tous passés ; updater portable passé. CLI inspection passive, parser et Cancel AppKit passés. Test menu rejetant l'action interne automatique AppKit d'un parent de sous-menu supprimé (assertion d'implémentation). Requête globale fixe non réussie, donc aucune lecture complète/autorisation interactive revendiquée | Conserver stderr/exit du superviseur pour établir la cause réelle, corriger puis requalifier ; sortie propriétaire pendant confirmation exercée indépendamment. Aucun tag/release ni remplacement PC |
| 2026-10-15 | Qualification 0.18.0 `9294701` | **CI38712517145 : menus/AppKit sept langues et app/dmg ARM/Intel réussis ; seule lecture globale échoue encore** | Confirmation native rendue/capturée et annulée, fermeture du propriétaire retire le helper ; requête globale fixe sort 126 après enfant exit25. Refus du PATH hérité sous Perl `-T` reproduit localement avec executable absolu : échec avant / succès après affectation littérale du PATH ; [contrat Perl officiel](https://perldoc.perl.org/perlsec#Cleaning-Up-Your-Path). Correction conserve `-T`, réinitialise l'environnement dans le programme et retire le mode utilisateur devenu inutilisé | Confirmer la lecture globale réelle après correction sur les deux Mac ; pas d'exception exit25 ni faux Allowed. AppKit/Finder/Exit ne valent pas consentement OS interactif ou capture physique |
| 2026-10-16 | Complément P1-MAC-APP-DMG et revue sécurité | **Qualification `b84976f` en cours, aucune nouvelle publication** | Déplacement/remplacement même build, préférences/historique, autostart possédé enabled/dormant, refus POSIX/retrait et clavier AppKit ajoutés au vrai smoke. Revue : assertion de succès Exit déplacée après disparition effective UI/processus ; Foundation Process crée un autre PGID, donc lancement ordinaire profiles remplacé par posix_spawn à groupe hérité, argv/env littéraux et waitpid possédé ; régression PGID réelle ajoutée, suivi sécurité sans nouveau finding | Compiler/exercer la source corrigée sur Mac ARM/Intel avant tag. Pages DMG sept langues préparées et vrai rendu desktop/mobile/clavier essayé en export privé seulement ; ne publier les liens qu'après disponibilité réelle des assets |
| 2026-10-16 | Qualification complète des parcours app/menu `b84976f` | **CI38859102456 quatre OS réussie ; deux app-smoke.json status=passed (pas exit77)** | ARM/Intel : préférences/historique octets+mode+inode conservés, refus chmod500 puis récupération, trois remplacements entiers du même build (première app publique), déplacements, vrai Exit, autostart explicitement possédé chargé/dormant et refresh/retrait, nettoyage exact du login runner, clavier AppKit direct passés | Pas d'upgrade inter-version app inventé, pas de consentement OS humain ni capture matérielle. Confinement final : waitid WNOWAIT observe l'helper sans libérer son PID, termine son groupe puis reap ; source revue sans finding, CI native finale avant tag |
| 2026-10-16 | CI38860306552 `99ff106` — garde de référence | **Échec Intel sur référence publique immuable 0.16.1, avant candidat ; publication toujours retenue** | Source/revue PGID, profils, menu, app et updater ciblés passent sur les deux Mac. Rapport ressources : baseline top/controlled_refresh_burst_slow_consumer ne quitte pas après Q ; cleanup SIGTERM termine en TimeoutExpired. Le harness gardait le wait20 sans SIGKILL final. Correction : escalade bornée sur groupe du Popen encore possédé/non reaped, fermeture PTY en finally et fixture native qui ignore SIGTERM puis doit sortir par SIGKILL | L'échec fonctionnel de référence reste failed et comparaison non qualifiée ; aucun retry Q, délai produit augmenté, gate candidat ou erreur de cleanup masqués. Exercer le nouveau cleanup avant tag |
| 2026-10-16 | Confinement final `85fc7c9` | **CI38861126385 quatre OS réussie** | ARM/Intel : vrai PGID profiles hérité, scopes/ownership/refus d'entrée, inspection passive unknown, Cancel, sortie propriétaire, lecture globale préautorisée, menus/app complets et updater réussis ; waitid WNOWAIT et nettoyage pré-reap réellement compilés/exécutés | Dernière fixture de cleanup SIGTERM-resistant en cours sur `7d9fcdb`/CI38862636413 ; pas encore de tag ni de lien DMG public. Aucun consentement OS humain, capture matérielle ou budget RAM inventé |
| 2026-10-16 | Qualification finale et release 0.18.0 | **Publié stable/latest**, tag annoté immuable `a2b19b6` | CI38863239649 et release38865573731 quatre OS réussies ; JSON app ARM/Intel passed, PGID réel, Cancel/sortie propriétaire, clavier et cleanup SIGKILL -9 exercés. DMG Apple Silicon/Intel inclus, neuf assets publics | Aucun Developer ID/notarisation, capture physique, consentement OS humain ou objectif RAM acquis ; PC 0.17.2 non modifié |
| 2026-10-17 | Vérification publique 0.18.0 / distribution | **Neuf téléchargements/digests/tailles et huit sommes/attestations exactes vérifiés** | Source complète `a2b19b6226ded2f32debb3147d3a3e9ef99116a4`, workflow release et tag ref vérifiés avec runners self-hosted refusés ; archive source1 183 889 octets/SHA380bb4eb pour recettes. Premier run Windows39024728463 arrêté avant swap par quota API anonyme HTTP403, pas par manque UAC | Attente migrations publiques Unix39024728377 et Windows sur runner jetable après prérequis quota ; publier/vérifier liens Pages réels. Les recettes ne valent pas publication du tap/AUR |
| 2026-10-17 | Migrations publiques 0.18.0 / site DMG | **Qualifiés**, Windows39025271101 et Unix39024728377 réussis ; Pages live source `f7dcdb1` | Windows : vraies anciennes/nouvelles ZIP attestées, update puis no-op aux hashes exacts, aucun tray ni contrôle lancé ; warning backup mappé conservé. Unix : Linux/ARM/Intel et bootstrap Mac passés. SiteCI39025249706 verte, quatorze onglets DMG visibles HTTPS desktop/mobile sept langues, vrais liens/canonical et Tab ARM→Intel vérifiés | PC 0.17.2 préservé ; P1-UPGRADE stdio actif reste ouvert. Arch39025249674 construit/installe0.18 mais fixture attend encore0.17.2 : constante retirée, version lue depuis recette, requalification requise. Workflow Windows de preuve jetable retiré après succès |
| 2026-10-17 | Qualification native Arch 0.18.0 `8782e5f` | **Réussie**, CI39025771494/report source_version0.18.0 statuspassed | .SRCINFO/build source locked, C/Rust SPA réel, PipeWire privé/PTY, pacman install/reinstall/vrai pkgrel-upgrade/remove ; préférences/historique/autostart externe désactivé préservés, aucun bootstrap privilégié. `install.sh` Pages HTTPS/textplain/SHA b734 exact ; marqueur de version du smoke dérivé de recette | Aucune publication AUR/tap ni nouvelle permission/installation PC ; limites macOS API, consentement OS humain, matériel et mémoire inchangées. Tag0.18.0 reste immuable |
| 2026-10-17 | P0-DEPS-ZIP — audit postpublication | **CI39025249652 échouée à l'audit Windows**, Linux/ARM/Intel réussis ; source corrective0.18.1 en qualification | RustSec0367/0368/0369 émis le17/10, ZIP8.6.0 affecté/patched>=9.0.0 ; self_update1.3.1 publié ce jour utilise ZIP9. Manifest/lock migrés vers ZIP9.0.4 et zipsign-api0.2.2, aucune exception d'audit. MCW sélectionne deux exécutables Windows via extraction single-file refusant les symlinks ; pas d'exploitation MCW prouvée | Vérifier audit et véritable extraction de ZIP publique, puis CI4OS/tagcorrectif/9assets/provenance/migrations/site. Stable reste0.18.0 pendant qualification ; PC0.17.2 préservé |
| 2026-10-17 | P0-DEPS-ZIP — qualification locale 0.18.1 | **Audit sans avis signalé et vraie extraction native réussis**, source non publiée | 462 dépendances/1334 avis, fmt/Clippy/167 tests (2 ignorés), build release/CLI réelle0.18.1 ; extracteur single-file self_update1.3.1/ZIP9.0.4 sur ZIP publique0.18 attestée, deux hashes exécutables exacts. Source de smoke jetable retirée ; workflow public Windows paramétré prépare update/no-op/réparation d'un compagnon absent sur runner jetable | CI4OS et releasecorrective avant site/recettes ; aucune exécution des anciens exécutables extraits ni mutation des contrôles PC |
| 2026-10-17 | P1-TRAY-PROFILES — extension demandée à tous les OS | **Inscrit, Windows/Linux non implémentés dans ce lot** | L'utilisateur précise que les quatre choix directs de profils doivent être proposés dans tous les menus natifs, pas seulement macOS. État macOS0.18.0 livré conservé ; lot multiplateforme et critères dédiés ajoutés | Implémenter/qualifier Windows et Linux avec choix coché, persistance, priorité config explicite et souris/clavier/sept langues ; ne pas présenter la seule preuve Mac comme parité acquise |
| 2026-10-17 | Release corrective P0-DEPS-ZIP 0.18.1 | **Publié stable/latest**, tag annoté immuable `034d2c1` | CI39027055886 et release39028452282 quatre OS réussies, JSON app ARM/Intel0.18.1 passed ; neuf téléchargements/digests/tailles et huit sommes/attestations source/ref/workflow contraints vérifiés. SBOM public : self_update1.3.1/ZIP9.0.4/zipsign-api0.2.2. Archive source1 191 879octets/SHA c2aa153f pour recettes | Migrations publiques Windows39029450035 et Unix39029450053, puis déploiement Pages/recettes à consigner ; aucune exception audit, modification du tag0.18.0 ou installation PC |
| 2026-10-17 | Migration Windows publique corrective 0.18.1 | **Réussie**, workflow39029450035/report passed | Vrais téléchargements attestés0.18.0→0.18.1, update puis no-op et réparation du compagnon délibérément retiré dans le runner privé ; paire exacte aux hashes publics à chaque étape, décodeurZIP9 publié réellement exercé. Aucun tray ou contrôle PC lancé ; warning backup ancien CLI mappé conservé | Unix39029450053 et déploiement Pages/recettes à consigner ; P1-UPGRADE handoff d'un tray actif reste ouvert, aucune restauration de quarantaine |
| 2026-10-17 | Pages corrective 0.18.1 / recettes | **Publié HTTPS et vérifié**, source web `cc0d3bf`, siteCI39029747260 réussie | Quatorze onglets macOS réels desktop1280/mobile390 sur sept langues : deux DMG0.18.1 visibles, source034 complète/canonical/absence de tokens et Tab ARM→Intel vérifiés ; captures françaises relues, navigateur fermé. `install.sh` HTTPS SHA b734 exact ; recettes source034/SHA c2aa publiées dans dépôt | Attente migrations Unix39029450053 et qualification Arch39029747230 ; aucune soumission tap/AUR ni nouvelle permission/installation PC |
| 2026-10-17 | Migrations Unix publiques correctives 0.18.1 | **Réussies**, workflow39029450053 trois OS vert | Linux/macOS ARM/Intel, installer bash/zsh/fish et migration0.18.0→0.18.1, bootstrap/updater standalone Mac réels passés ; aucun exit77 substitué à une preuve de migration, pas de capture ou permission humaine revendiquée | Qualification Arch39029747230 et intégration39029747275 encore suivies ; politiques/contrôles PC0.17.2 et tags antérieurs préservés |
| 2026-10-17 | Qualification Arch corrective 0.18.1 | **Réussie**, CI39029747230/report source_version0.18.1 statuspassed | Source034/SHA c2aa, .SRCINFO/build locked, PipeWire privé/PTY et pacman install/reinstall/pkgrel-upgrade/remove ; préférences/historique/autostart externe désactivé préservés, aucun bootstrap privilégié. Rapports et paquets de preuve conservés, publication=not_submitted | Aucun tap/AUR soumis, ni changement PC ; CI intégration39029747275 encore suivie, tag0.18.1 non déplacé |
| 2026-10-17 | Clôture corrective 0.18.1 / intégration | **Lot livré et vérifié de bout en bout**, CI39029747275 quatre OS réussie sur métadonnées `cc0d3bf` | Source034/tag0.18.1 immuables ; audit sans ignore, neuf assets/huit attestations/SBOM corrigé, Windows update/no-op/réparation, Unix trois OS, app native ARM/Intel, Arch et Pages sept langues qualifiés. Dernier commit de ce bilan documentaire distinct du tag | P1-TRAY-PROFILES tous OS inscrit : Windows/Linux restent à faire ; handoff stdout d'un tray actif, signatures/notarisation et limites matérielles/API restent explicites. Aucun changement de la paire PC0.17.2 ou des contrôles demandé |
| 2026-10-17 | P1-MAC-UX — retours ami macOS pour la prochaine version | **Quatre demandes inscrites, non implémentées** | Statut peu clair, absence de vraie fenêtre de consultation, entrées tray potentiellement superflues à examiner, ajout d'un À propos présentant brièvement MicCamWatch ; section et critères dédiés ajoutés | Réaliser et vérifier les quatre surfaces natives ; choix précis des entrées à retirer après revue, profils directs tous OS conservés dans P1-TRAY-PROFILES |
| 2026-10-17 | SITE-DEFAULT-EN / SITE-HERO — correctifs | **Source corrigée et preview qualifiée ; publication HTTPS en cours** | Export réel racine accueil/404 anglais et quatorze pages localisées ; quatre tests Node ; Chromium français sans redirection, choix `/fr/`,404 root et racine sans JS ; 133 scénarios de lisibilité/rectangles sur sept langues, dont reflow équivalent125/150 % et texte200 % | Publier, observer le vrai HTTPS et consigner CI/archives ; ni version binaire ni installation/contrôle PC modifiés |
| 2026-10-17 | SITE-DEFAULT-EN / SITE-HERO — clôture HTTPS | **Publié et vérifié**, source web `277dd04`, CI site39032505628/Cloudflare5ff8f167 réussis | 133 scénarios réels sur sept langues et captures russes relues ; racine200 anglaise avec/sans JS et navigateurs français/russe,404 réelles anglaise/française/russe, sélecteur et installation exercés ; métadonnées et nouvelle archive CI vérifiées | Aucun reste sur ces deux correctifs ; P1-MAC-UX et profils directs Windows/Linux restent pour la prochaine version. Binaires/tags/PC0.17.2 et contrôles inchangés ; zoom qualifié par reflow CSS/DPR, pas par menu navigateur |
| 2026-10-18 | TRAY-WIN / P1-UPGRADE / SITE-COPYRIGHT — reprise | **Source corrective intégrée, qualification native en cours** | Copyright approuvé sans tiret, licence MIT inchangée hors nom, quatre tests Node et32 footers réels qualifiés en preview ; lancement NUL insuffisant établi, relance native sans héritage/détachée et EOF avec successeur vivant réussis ; fixtures vrai compagnon et Explorer réservées au runner hébergé | Consigner qualification finale/CI native et copyright HTTPS ; pas de version/tag/release ni de changement à l'installation ou aux protections PC |
| 2026-10-18 | SITE-COPYRIGHT — livraison HTTPS | **Publié et vérifié**, source `9b52e43`, siteCI39127360873 et Cloudflare d1e666cf réussis |32 parcours navigateur réels1280/390 : huit accueils200 et huit routes inconnues404 dans leurs langues ; copyright visible exact sans tiret, lien MIT et licence publique avec même nom, anglais racine conservé ; capture footer relue et navigateur/preview fermés | Aucun reste copyright ; permissions MIT et binaires publiés inchangés, qualification native séparée en cours |
| 2026-10-18 | CI39127360852 — correctifs Windows | **Handoff vrai compagnon réussi ; preuve Explorer échouée**, trois Unix natifs réussis |170 tests locaux/Clippy/fmt/build et vraie CLI, preuve EOF contrôlée et mode vrai compagnon hébergé réussis ; audit Windows passé. Explorer enregistre le bouton neutre mais aucune infobulle UIA observée au premier hover ; pas de capture visuelle inventée | Pilote visuel corrigé pour visibilité/mouvement natif et diagnostics ; requalifier la surface avant validation TRAY-WIN, MSI non exécuté après l'échec ; pas de tag ou installation PC |
| 2026-10-18 | CI39129154200 — pilote Explorer | **Vrai compagnon à nouveau réussi ; hover non qualifié**, trois Unix réussis | Capture du desktop jetable relue : bon bouton neutre et curseur aux coordonnées attendues, aucune infobulle de rôle UIA ToolTip recueillie ; aucun résultat visuel substitué. Le pilote cherchait récursivement depuis tout le desktop, contrairement au périmètre recommandé par Microsoft | Recherche limitée aux fenêtres Shell top-level puis à leurs sous-arbres ; popup rendu distinct du bouton exigé, sans imposer un rôle UIA unique aux variantes XAML ; requalification en cours |
| 2026-10-18 | CI39131396247 — popup Shell | **Handoff encore réussi ; popup non qualifié**, trois Unix réussis | Les seuls noms recueillis restent ceux du bouton, aucune fenêtre de popup distincte acceptée ; le pilote ne transforme pas cette absence en succès. Nouvelle instrumentation passive de la callback réelle de l'icône et capture une seconde après entrée du pointeur | Déplacement SendInput désormais non nul et non coalescé ; consigner callbacks/frames du runner pour distinguer injection absente et popup non rendu. Aucun changement des contrôles ni observation du tray PC |
| 2026-10-18 | CI39133674447 — réception native du hover | **Handoff réussi ; aucune callback Shell reçue**, trois Unix réussis | Vraies frames à1 seconde et après attente relues : pas de popup ; trace de la fenêtre propriétaire `callback_count=0`, contrairement à une simple difficulté de rôle UIA. Les mouvements SendInput sont acceptés mais cette seule acceptation ne prouve pas leur réception | Vérifier la frontière UIPI du runner élevé : seule la callback inerte de la fenêtre de fixture est autorisée, jamais une fenêtre/message produit ; contexte d'élévation et hit-test natif consignés pour la prochaine preuve. PC inchangé |
| 2026-10-18 | CI39135806841 — contexte du runner | **Hover toujours non qualifié**, handoff et trois Unix réussis | Runner effectivement élevé ; le hit-test natif vise Explorer, mais la callback reste à zéro malgré le filtre limité à la fixture. Probe local temporaire d'une fenêtre masquée possédée : message posté reçu, aucun Shell/utilisateur touché ; probe retiré après preuve | L'activation du flyout passe désormais par clic natif, et le bouton doit correspondre au rectangle Shell de l'identité exacte HWND/uID (pas seulement au nom), afin d'écarter caches/icônes anciennes. Requalifier, sans déclarer UIPI cause démontrée |
| 2026-10-18 | CI39138433664 — identité exacte de l'icône | **Popup initial toujours non qualifié**, handoff et trois Unix réussis | Le rectangle de l'identité Shell HWND/uID correspond exactement au bouton survolé ; le hit-test vise Explorer et les callbacks restent nulles. Ni icône ancienne ni simple nom de bouton ne constituent désormais la preuve | Le pilote collecte tous les états Active/Partial/Error/Ready/Idle et leurs menus/détails avant d'échouer globalement si un popup manque ; aucun cas manquant n'est transformé en succès, aucune portée d'acceptation abaissée |
| 2026-10-18 | P1-UPGRADE-ACTIVE — retour utilisateur Windows | **Inscrit en priorité corrective, non corrigé** | Deux refus ownership Busy avec tray actif, réussite0.18.1 après Exit manuel puis `mcw tray`, selon la console utilisateur ; warning backup accès refusé distinct. Aucun nouvel essai sur son PC | Coordonner update/frontend de la même installation sans arrêt manuel ni contournement des protections ; vraie migration native à qualifier ; autres plateformes à examiner, pas présumées affectées |
| 2026-10-18 | P1-STARTUP-PASSIVE / P0-CAM-RESUME — précisions utilisateur | **Choix et défaut inscrits, non corrigés** | Démarrage passif sûr choisi ; relevé caméra bloquée/garde inactive et contournement Autoriser/Rebloquer rapportés. Aucune action sur les appareils du PC | Conserver l'intention existante, réarmer sans Allow intermédiaire avec consentement requis et qualifier le premier état de session ; ne pas confondre zéro capture, permission et ownership |
| 2026-10-18 | P0-CAM-RESUME / P1-STARTUP-PASSIVE — implémentation source | **173 tests Windows passés, 3 natifs ignorés, Clippy/build réussis** | Reprise seule sans Allow, refus d'une intention Allow récente, restauration conservée ; première lecture de session passive. Nouveau scénario natif réel ajouté mais pas encore exécuté ; aucune interaction avec le tray ou les appareils installés | Qualifier la nouvelle paire en CI isolée ; UAC humain et matériel restent non exercés ; source non publiée en release |
| 2026-10-18 | P1-UPGRADE-ACTIVE — audit historique et diagnostic | **Pas de verrou global inutilisé démontré** | Code pertinent identique v0.17.2/v0.18.1 ; propriétaires natifs réels seuls détenteurs des ressources ; arrêt du consommateur peut résoudre une restauration automatique. 14 régressions transactionnelles passées après précision des erreurs | État des tentatives historiques inconnu ; ne pas transformer Exit en contournement automatique ni prétendre la migration complète corrigée |
| 2026-10-18 | CI39140614452 — qualification du popup Windows | **Échec natif conservé, pas de certification** | Trois jobs Unix réussis ; vrai handoff/EOF du tray et protections zéro périphérique réussis. Popup startup absent ; collecte suivante interrompue par le nom UIA composé `miccamwatch` + statut courant, pas par un statut périmé | Pilote corrigé pour cette composition exacte et diagnostic des noms de desktops/hover time ; aucune preuve popup inventée |
| 2026-10-18 | Site — exemple SHA-256 macOS signalé par capture | **Publié et vérifié en HTTPS** | Commit97bedca, CI site39154738282 réussie ; vrais noms Apple Silicon/Intel v0.18.1 ; quatre tests et export privé passés, puis huit routes ×1280/390 sur le vrai site, 16 cas HTTPS passés sans placeholder ni débordement ; russe inclus, racine anglaise | Aucun reste sur ce défaut ; pas de changement aux archives ou binaires stables |
| 2026-10-18 | P1-STARTUP-PASSIVE — obligation différée pendant Unknown | **Régression reproduite puis corrigée en source** | Revue : une vraie transition Locked différée pouvait être perdue pendant une lacune ou une fin d'action Unknown. Test permanent échoué avant correction, passé après ; 174 tests, Clippy et release build réussis | L'obligation existante attend un état connu sans inventer d'arête ni redemander après consommation ; Unlock ou retrait explicite de politique l'annule. Qualification native du commit final à réception |
| 2026-10-18 | CI39155502319 — vraie reprise caméra sur runner zéro périphérique | **Trois Unix réussis ; Windows arrêté par sa barrière d'inventaire pendant la reprise** | Tests ordinaires, build, EOF contrôlé, TUI et readiness natifs réussis ; arrêt dans `await_state` avant qualification du nouveau helper, aucun popup/MSI ensuite. Le constructeur de statut du helper publie `unknown_devices:1` tant que son observation n'est pas établie | Pilote séparant désormais inventaire PnP direct (Camera/Image présents, y compris désactivés) et statut transitoire ; absence/erreur PnP jamais assimilée à zéro, Unknown jamais succès. Smoke contrôlé des barrières passé ; scénario réel à relancer sur cette source |
| 2026-10-19 | CI39202479245 — reprise caméra réelle puis contention IPC | **Reprise sans Allow passée ; job Windows toujours en échec distinct** | `camera_guard_recovery` confirme commande du vrai tray, nouveau helper authentifié, Block retenu et micro préservé ; sept observations transitoires attendues, PnP connu vide. Arrêt ultérieur sur `unmute` : toutes instances du pipe occupées, erreur231 ; cleanup sans erreur, trois Unix verts | Ni popup Shell ni MSI exercés après cet arrêt ; pas de succès global annoncé, aucun périphérique utilisateur touché |
| 2026-10-19 | P0-WIN-IPC — admission bornée du canal natif | **Régression native échouée avant/passée après ; 175 tests, Clippy et release build passés** | Deux clients réels sur un pipe isolé : contrôle appliqué une seule fois, lecture suivante admise, connexion occupée refusée à sa deadline. `WaitNamedPipeW` uniquement sur231 avant envoi ; aucun replay et mêmes pairs/capacités | Scénario complet hébergé à réception ; distinct du Busy258 d'ownership de l'updater, jamais contourné |
| 2026-10-19 | CI39205263872 — cycles natifs complets et limite du survol | **Garde/reprise/libération, ressources et EOF réels passés ; échec visuel strict conservé** | Trois Unix verts ; Windows175 tests, Clippy/audit/build, TUI, readiness, reprise sans Allow et libération micro, ressources et vrai compagnon avec EOF réussis. Sept menus/détails réels, copie UTF-16 identique7/7, largeur maximale297px sous borne400px ; nouvelle action de reprise visible sur capture | Huit infobulles Explorer non observées, callbacks nuls ; desktops Default identiques, hover400ms, hittest Explorer et rectangle exacts. MSI non exécuté après ce refus ; pas de certification globale ni matérielle |
| 2026-10-19 | TRAY-WIN — diagnostic ciblé de l'entrée du runner | **WM_MOUSEMOVE réellement reçu, popups toujours non qualifiés** | CI39208416398 : fenêtre inerte possédée, `move_count:1`, coordonnées client82/29 identiques à la cible, `routed:true`. Les cycles natifs de protection/EOF et les trois Unix passent encore ; aucun popup sur huit états, callbacks Shell nuls | Une absence générale de déplacement n'explique pas ce run ; qualifier séparément les clics injectés et les callbacks de l'icône, sans changer le produit ni assimiler le probe à une infobulle |
| 2026-10-19 | TRAY-WIN — séparation déplacement/clic natif | **Instrumentation complémentaire154a784, production inchangée** | Compteur WM_LBUTTONUP de la fenêtre inerte et clic ciblé sur sa seule icône HWND/uID après échec du survol ; snapshots distincts, critères popup inchangés.37 régressions frontend et Clippy passent ; pilote parsé/interop compilée sans entrée locale | Exercer sur runner isolé ; un clic reçu ne prouvera pas une infobulle visible |
| 2026-10-19 | Qualification locale — persistance du handoff micro | **Échec existant observé et non masqué** | Suite complète :174 passent,1 échoue,3 ignorés ; `matching_shutdown_handoff_survives_reopen_but_cannot_release_new_manual_intent` échoue sur `reopened.save`/MoveFileExW avec0x80070005 dans un répertoire de test privé. Code de persistance inchangé, cause non établie ; cas non rejoué pour effacer l'échec | Ne pas supprimer le test, élargir les droits ou contourner l'antivirus ; distinguer cet incident local de la compilation et du diagnostic d'entrée. La CI suivante conserve la suite complète |
| 2026-10-19 | CI39211420515 — borne des mesures natives | **175 tests et trois Unix passent ; scénario Windows refusé avant le diagnostic Shell** | Reprise caméra sans Allow et libération abouties, mais une ligne sans garde caméra à73.328s est incluse dans la phase terminée exactement à73.328s. `proof_complete:false` et erreur conservés ; aucun clic/popup/MSI qualifié dans ce run | Correction du pilote7892717 : indices d'acquisition figés sous le verrou du sampler, pour chaque phase et la mesure globale ; aucune ligne écartée selon ses rôles, aucune protection de production modifiée |
| 2026-10-19 | Mesures natives — validation des bornes d'acquisition | **Reproduction avant / smoke après passé sans action native locale** | Horloge à égalité : ancien sélecteur prend5 lignes au lieu de3, nouveau prend les3 acquises dans la phase ; une garde absente à l'intérieur reste détectée. Vrai `Sampler.finish` exercé : médiane de phase20, pic global9999 conservé, scripts complets compilés | Qualification hébergée de cette source à réception ; ne pas transformer les résultats synthétiques de bornage en preuve matérielle ou de popup |
| 2026-10-19 | CI39213072436 — bornes natives qualifiées, défaut distinct de réponse micro | **Phases TUI/tray valides après correction ; suite native Windows toujours en échec** | Indices verrouillés :802 et801 échantillons, deux gardes et frontend présents sur toutes les lignes de phase. Reprise caméra sans Allow passée ; plus tard `unmute` reçoit `microphone owner response deadline` (attente acteur2s). Cleanup sans erreur ;175 tests ordinaires et trois Unix verts | Cause précise du délai non établie après lecture transport/acteur/release SDK ; intention peut déjà être persistée avant l'attente, résultat SDK inconnu : pas de replay, allongement arbitraire ou succès inventé. Popup et MSI non exercés dans ce run |
| 2026-10-19 | CI Windows — qualification visuelle indépendante | **Commit49d72ab : condition explicite, aucun échec ignoré** | Pilote inerte sans SDK lancé si compilation/tests Windows réussis et run non annulé, même si le scénario privé de gardes échoue ; même convention que les qualifications macOS indépendantes. YAML parsé et prérequis vérifiés | Mesurer clic/callback/popup sans attendre une correction non démontrée du délai de réponse ; un résultat visuel ne blanchira pas la qualification des gardes ni le job global |
