# Roadmap technique MicCamWatch

Dernière mise à jour : **2026-10-24**. Responsable : lead architecture/développement.
Base publiée : [v0.20.0 stable/latest](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.20.0), source `a26522028eafe02591f58c03ceb5c367f2d5d60b`, tag annoté `547f6185a0579876041d1ae7ebcdd6feccdcb5cf`, releaseID 420936561, publiée le 22/10 à 16:13:39Z. [Release native 39807585260](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/39807585260) réussie : neuf assets publics taille/digest API, huit entrées SHA256SUMS et huit sujets de provenance signée SLSA v1 vérifiés ; Windows EXE/MSI non signés, macOS ad-hoc.

**Publication 0.20.0 effectuée le 22/10 :** les fichiers publics, leurs sommes et leur provenance sont vérifiés. Le site et les pins du dépôt sont alignés après cette vérification ; aucune publication externe Homebrew/AUR effectuée. Les essais PC autorisés ont utilisé des installations de test isolées puis retirées, sans remplacement de l'installation habituelle ni commande de protection micro/caméra.

**Prochain lot en cours : P1-ARM, source `0.21.0` non publiée**, sur la branche
`work/linux-arm64-021`. Construction et qualification Linux ARM64 natives sur
Ubuntu22.04/glibc2.35, contrôles ELF et frontières d'installation conservés.
La stable0.20.0 et son site restent inchangés ; un PASS en VM ne certifiera pas
le matériel d'un Raspberry Pi/SBC.

**Historique du candidat0.19.0 non publié livré avant cette autorisation :**
paquets actualisés du commit `535bfc60d768976b98486c327fe7d1cca816980d` disponibles dans
`C:\Users\stcui\Downloads\MicCamWatch-0.19.0-candidat-535bfc6`.
Build natif quatre OS réussi, huit SHA-256 et huit sujets de provenance signée
vérifiés avec workflow/ref/source/signer exacts et runners hébergés ;
ZIP Windows exécuté uniquement avec `--version`, MSI lu sans installation.
Inclut le correctif de diagnostic micro3aff04f, sans reprise forcée d'une garde incompatible.
Lors de cette livraison candidate, aucun tag/publication/installation automatique ; la stable/site/recettes restaient0.18.1.
Limites TRAY-WIN et P1-UPGRADE-ACTIVE conservées ; EXE/MSI non signés Authenticode.

**Retour utilisateur Windows du20/10 :** « ça a l'air d'être bon.
en tout cas pour windows ». Retour manuel global positif après livraison du
candidat ; chemin exécuté et scénarios précis non détaillés. Ne pas étendre ce
retour à macOS/Linux, à tous les contrôles matériels ou à une qualification
automatisée du popup Explorer. Aucun nouvel essai PC déclenché pour le confirmer.

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
| P0-CAM-RESUME | **Publié0.19.0**, reprise réelle qualifiée sur runner zéro périphérique : garde relancée depuis le vrai tray sans Allow intermédiaire ; cause IPC conservée | Élevée | CI39217101436 : cycle complet passé, nouveau helper authentifié, Block et micro préservés ; UAC humain et matériel non certifiés |
| P0-WIN-IPC | **Publié0.19.0**, scénario natif complet passé : concurrence des clients du pipe admise avec attente bornée | Moyenne | CI39205263872 : vraie libération micro après garde/tray passée ; connexion seule attend3s, pairs conservés, aucune commande rejouée ni mutex d'ownership contourné |
| P0-WIN-OWNER-IDLE | **Publié0.19.0**, cycle du vrai broker qualifié : arrêt coordonné avec les requêtes admises, sans hausse des délais ni replay | Élevée | bb70dcf, régression ciblée et contre-exemple concurrent sans SDK ; CI39217101436 proof_complete:true, libération et EOF réels passés. Cause unique du délai historique non certifiée |
| P1-MAC-MENU | **Publié 0.18.0, AppKit réel qualifié** : quatre profils directs cochés, marque MCW template/badges, Details au-dessus d'Exit, sept langues et `--config` prioritaire | Moyenne | IPC desktop v2 frontend/helper appariés ; captures rendues et vraies fenêtres relues, données de capture simulées explicitement |
| P1-MAC-UX | **Consultation publiée0.20.0 ; refonte structurée0.21.0 en cours** à la suite de la nouvelle capture utilisateur | Moyenne | Remplacer le mur de diagnostics initial par Vue d'ensemble/Réglages/Diagnostics, état micro/caméra et accès actuellement observés, contrôles de préférences réels, présentation AppKit lisible et sept langues. Diagnostic causal séparé : corriger les défauts établis, pas masquer les limites macOS. Qualification native Intel/ARM à refaire pour ce nouveau lot |
| P1-GUI-ALL | **Demandé et en implémentation0.21.0**, Windows/Linux en parallèle de macOS | Élevée | Même organisation Vue d'ensemble/activité actuelle/Réglages/Diagnostics/À propos, composants natifs Win32/AppKit/GTK3 et capacités réellement disponibles. Linux charge GTK3 seulement à l'ouverture GUI pour conserver CLI/headless ; Windows garde autostart silencieux et ouverture explicite depuis Démarrer/tray. Qualification native des trois OS et des deux architectures Linux/Mac nécessaire |
| P1-TRAY-PROFILES | **Publié0.19.0, qualifié natif Windows/Linux** : quatre profils directs cochés, profil Windows également visible dans le libellé parent | Moyenne | Correctif62b550e, CI39298930148 :21 cas Windows passent, vrais rendus Profil — Réunion/Développement et priorité --config relus ; trois Unix verts. Collecteurs et protections matérielles hors fixture |
| SITE-PAGES | **Publié et vérifié HTTPS0191**, source web89a7c8a : racine anglaise, russe et liens publics191 | Moyenne | Export privé huit accueils/quatre tests Node ; vrai navigateur rooten et /ru/ langru, huit liens191 et copyright vérifiés. Captures171 historiques non relabellées ; pas de fonctionnalités0.20 présentées comme déjà disponibles en0191 |
| SITE-TOKENS | **Corrigé sur Pages, vérifié après déploiement `f1344bf`**. Build Git exécuté depuis racine dépôt, output `site-dist`, version/source/checksum réels ; source HTML brute n'est plus un entrypoint | Faible à Moyenne | Gardes build, source unique des faits, aucun token résiduel |
| SITE-L10N | **Publié et exercé en production** : en/fr/de/es/ja/zh/ru, 227 messages chacun, titres longs et navigation mobile repliables ; DMG localisés conservés | Moyenne | CI site39032505628 ; 133 scénarios HTTPS sept langues, racine sans JS/français/russe et404 root/fr/ru observées ; images171 non relabellées, CSP inchangée |
| SITE-HERO | **Publié et vérifié sur HTTPS**, source `277dd04` : titres longs autorisés à revenir à la ligne et navigation mobile repliable ; texte russe intégral, aucune troncature | Faible | 133 scénarios Chromium sur sept langues, rectangles de texte contre carte, largeurs320–1568, reflow équivalent125/150 % et texte200 % ; captures russes desktop/mobile relues |
| SITE-DEFAULT-EN | **Publié et vérifié sur HTTPS**, source `277dd04` : accueil/404 racine anglais sans redirection de langue ; sept routes et sélecteur conservés | Faible | `/` conservé avec/sans JS et navigateurs français/russe ; métadonnées en_US/canonical `/en/`, choix français, cible installation et404 réelles root/fr/ru exercés |
| SITE-COPYRIGHT | **Publié et vérifié sur HTTPS**, source `9b52e43` : `© 2026 Roman Cuisset` sans tiret sur accueil/404 de toutes les langues ; même nom dans LICENSE | Faible | Quatre tests Node et32 surfaces réelles Pages desktop/mobile ; huit accueils200 et huit véritables404 localisées, lien MIT/public LICENSE, permissions et anglais racine conservés |
| SITE-INSTALL-404 | **Corrigé et exercé en production**. `/install.sh` Pages sert le script exact ; vrai curl serveur ASUS, installation native 0.17.2 puis uninstall possédé sous HOME temporaire | Faible à Moyenne | Maintenir payload/routage et digest ; aucun autostart/tray/protection/root-helper activé |
| P1-RAM | Publié 0.17.0, validé logiciel/natif : bornes et mesures complètes, 15 Mo non atteints, scénario composé Windows ~43 Mo | Moyenne | Maintien des preuves/limites canoniques ; 30 Mo reste une référence |
| P1-RESTORE | Publié 0.17.0, validé logiciel/natif : propriétaire Windows, états opposés/hotplug/partiel simulés, release natif réel sans devices | Élevée | Pas de certification matérielle ou déni exclusif/ASIO |
| P1-HB | **Six parcours natifs0.20.0 qualifiés, non publiés** ; recette publique0191 source3dd/SHAb044 inchangée | Moyenne | ARM39768723173 et Intel39774103433 : source201e, baseline0161, intents chargé/dormant/désactivé, refresh explicite avant cleanup, vrais test/retrait/réinstallation. Aucun tap/core publié ni autorisation externe déduite |
| P1-AUR | Recette0191 source3dd/SHAb044 synchronisée ; qualification native complète0181 conservée ; publication AUR bloquée par accès SSH mainteneur | Moyenne | Requalification courante distincte des pins ; aucune soumission externe ni autorisation mainteneur inventée |
| P1-WG | Winget : à faire | Moyenne | Identité MSI et upgrades contrôlés |
| P1-MAC-APP-DMG | **Paquets publiés0.19.1**, DMG Apple Silicon et Intel, sommes/provenance vérifiées | Moyenne | Natif release191 et smokes publics bootstrap/CLI passent ; preuves Finder/Exit/autostart antérieures conservées, ad-hoc seulement, pas d'upgrade inter-version app inventé |
| P1-ARM | **Qualification logicielle native réussie**, source2998946/0.21.0 non publiée | Élevée | CI39832624207 cinq plateformes vertes ; Ubuntu22.04ARM/glibc2.35/PipeWire0.3.48, ELF183 des deux binaires, CLI/PTY/PipeWire/panel et fixtures d'installation/root exercés. Retrait natif39828453493 réussi sur quatre Unix. GUI ultérieure à qualifier séparément ; aucun SBC physique, ARMv7/32bits, musl ni budget global15Mo prouvé |
| P1-UPGRADE | **Correctifs EOF019 et deux hosts191 publiés, qualifiés publics** | Moyenne | CLI39419044223 et compagnon39419044530 : vrai caller191 mappé, compagnon019 réparé, paire publique exacte, EOF avec successeur vivant et no-op mêmePID/HWND passés. Anciens clients non corrigés rétroactivement ; protections/restaurations restent des refus |
| P1-UPGRADE-ACTIVE | **Tray actif sans protection qualifié public191 pour les deux hosts** ; transfert de protection active non promis | Moyenne | Réparation/no-op publics191 passent sans kill ni perte de contexte/préférences ; lease/foreign refusés sans stop dans cinq layouts natifs. L'ancien frontend PC0181 déjà dans le backup exige une bascule unique, cause du Busy historique non établie |
| P1-STARTUP-PASSIVE | **Publié019 Windows** : premier état de session connu = baseline passive ; les lacunes Unknown ne créent pas d'arête | Moyenne | Régressions initial Unknown/Locked/Unlocked et transitions ultérieures passées ; protections demandées/restaurations conservées ; aucune action sur le PC par l'assistant |
| P1-UPGRADE-CLI-HOST | **Publié0.19.1 et qualifié public**, reconnaissance des deux images exactes avant remplacement | Élevée | Natif cinq layouts + publicCLI39419044223 : réparation/no-op, config relative depuis CWD distinct et --langfr face au fichierde réellement observés dans menu natif ; aucun héritage des pipes, état privé conservé. Acquisition et paire exactes vérifiées ; pas d'upgrade par l'ancien client019 ni de politique de classification/matériel prétendus |
| P1-MAC-TRAY-STOP | **Corrigé source0.20.0, huit cycles natifs par architecture réussis** | Moyenne | 66574b1 remet explicitement le flux accepté en bloquant ; délais inchangés. 39579584237/source24303fe passe Intel et ARM : huit starts/readiness/stop propres et trois réouvertures par cycle, aucune errno22 ; sorties des descendants retenues par kqueue. Traces et échec release191 conservés. CI complète39579549269 verte quatre OS ; pas de réécriture d'asset191 |
| P1-UNINSTALL | **Parcours natifs qualifiés en source0.20.0 ; publication à faire** | Moyenne | Unix/Arch et six états Homebrew qualifiés ; PC Windows TokenElevation0 : retrait portable compagnon/CLI avec autostart, six refus et véritable Cargo passent. MSI : vraie installation perUser AssignmentType0, binaires020 exacts, commande msiexec au ProductCode réel puis retrait effectif avec données conservées ; identités de paquet/composants/raccourci/AppModel isolées pour préserver les deux MSI0190 existants. Tray191/PID5632, paire, démarrage, raccourci original, anciens produits et PATH identiques avant/après. Pas de migration des anciens MSI PC ni de contrôle physique déduits de ces preuves |
| P1-WIN-APP-ICON | **Qualifié natif MSI/Shell en source0.20.0 non publiée** | Faible | 39655798674 : vrai MSI AssignmentType0, paire installée exacte, 36 comparaisons16–256 réussies sur CLI/tray/Démarrer/Apps ; PNG relus. Retrait réel du produit et raccourci, entrées PATH brutes/type/ordre conservés. Payload MSI précédent exact réutilisé, pas de réinstallation PC ; pas de signature/version PE inventée |
| P1-MAC-MENU-ICON | **Implémenté, rendu natif vérifié Intel/ARM en source0.20.0 non publiée** | Faible | Marque20pt, marges réduites sans déformation ; image26×20 et vrai NSStatusBarButton42×22 sans rognage. Dix captures clair/sombre par architecture, cinq badges distincts ; étape AppKit39568140011 passe sur les deux Macs, captures relues. Finder/About inchangés ; ce PASS visuel ne clôture pas l'échec IPC du même run |
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
| TRAY-WIN | **Correctif publié019 ; retour utilisateur Windows positif** : identité initiale neutre, résumé courant borné, UTF-16 sans demi-surrogate | Moyenne | Retour global du20/10, scénarios précis non détaillés ; popup Explorer automatisé toujours non qualifié (CI39312572930). Aucun nouvel essai PC pour confirmer le retour |
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

**Fonction :** produire `aarch64-unknown-linux-gnu` et `miccamwatch-linux-aarch64.tar.gz` par build natif sur `ubuntu-22.04-arm`, au même niveau de baseline que Linux x64. Une éventuelle cross-compilation ne remplacerait pas les smokes natifs nécessaires avant publication.

**Modules Rust :** `src/platform/linux.rs`, `src/platform/linux/pipewire.rs`, `src/platform/linux/control.rs`, `src/platform/linux/procfs.rs`, `src/privacy/linux/usb.rs`, `src/privacy/linux/helper.rs`. Intégration : `Cargo.toml`, `vendor/libspa`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `.github/workflows/unix-install.yml`, `installer/install.sh`, `installer/install-camera-helper.sh` et `installer/tests/`.

**Contraintes :** compiler, bindgen et pkg-config doivent viser les bibliothèques/header ARM64 de la baseline, sans emprunter ceux de l'hôte x64. Maintenir glibc 2.35+ et l'ABI PipeWire/SPA documentée ; aucune promesse ARMv7/OS 32 bits/musl. Dans la source0.21.0, l'installateur ordinaire, le bootstrap root et les fixtures sélectionnent désormais l'architecture native ; les deux ELF doivent être 64bits little-endian, de type exécutable et d'ABI autorisée, avec `e_machine=183` sur ARM ou `62` sur x64 avant toute sonde. Les contrôles d'intégrité et les restrictions des tests privilégiés aux machines jetables sont conservés.

**Validation :** fmt/clippy/tests, vraie extraction et CLI/PTY sur ARM64, serveur PipeWire, installation/migration/uninstall ; smoke desktop uniquement avec session/panel réels. Une VM ou QEMU ne prouve pas le support matériel d'un Pi. Tester un SBC autorisé séparément : micro observable et, si disponible, webcam USB conforme. Les caméras CSI/non-USB restent hors contrôle USB ; un SBC headless n'exige pas un tray fictif.

**État0.21.0 non publié :** source2998946 qualifiée dans CI39832624207, cinq plateformes réussies. ARM natif Ubuntu22.04/glibc2.35/PipeWire0.3.48 : compilation/fmt/clippy/tests, roundtrip SPA-C, ELF64LE183 CLI/helper et bindings natifs, extraction/CLI/PTY, vrai PipeWire/capture virtuelle, six pannes bornées avec reprise sans faux STOP, vrai panel/menu/profils/autostart et frontières d'installation/root passent. Installation/retrait gérés et vrai Cargo passent dans39828453493. Le premier run39828353569 reste un échec documenté (baseline ARM0161 inexistante), corrigé sans substituer x64 ni retirer les scénarios candidats. Migration de format au même candidat seulement, pas une migration publique ARM historique ; mesures candidates complètes mais comparaison historique et budget global15Mo non prouvés. La refonte GUI demandée ensuite nécessite ses propres nouveaux smokes. Aucun SBC physique qualifié.

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

**Cas PC du19/10 après essai manuel du candidat :** inventaire CIM et
QueryFullProcessImageName avec droit limité montrent une garde micro vivante
depuis `.cargo\bin\mcw.exe` et le tray candidat depuis Downloads. Le processus
enfant élevé pointe aussi vers le `mcw.exe` candidat ; sa ligne de commande n'est
pas lisible sans élévation, son rôle caméra n'est pas déduit du seul chemin.
Empreintes des deux images candidates identiques au ZIP0.19.0 vérifié.
L'intention existante est `requested:true`, `release_pending:false`,
`automatic_token:0`, `automatic_restore_pending:false` : ne pas interrompre
cette garde manuelle pour faire disparaître l'erreur du nouveau frontend.

Le `version mismatch` rapporté indique une garde incompatible avec le client.
Le namespace IPC est dérivé de SID/session et du chemin canonique de données
LOCALAPPDATA, pas du dossier de l'exécutable. Le mutex matériel est global :
son occupation seule ne prouve pas un « autre namespace ». Ne pas transformer
la coexistence de versions en manque UAC ; l'utilisateur signale la garde
caméra active après approbation. Aucune commande SDK, arrêt, mutation de fichier
d'intention, clipboard ou autostart pendant ce diagnostic.

**Libération demandée ensuite par l'utilisateur :** commande unique
`C:\Users\stcui\.cargo\bin\mcw.exe unmute`, exit0 ; réponse observée :
protection micro désactivée, SDK ACTIVÉ, un endpoint. Caméra inchangée.
Ce relevé ponctuel n'est ni une preuve de capture en réunion ni un état courant ;
aucune réactivation de protection ou expérience de pointeur ensuite sur son PC.

**Diagnostic corrigé en source3aff04f :** l'échec IPC initial est conservé
lors du refus d'activation/libération pour ownership occupé. Le mutex seul
n'affirme plus « autre namespace ». CI39312572930 exerce le refus réel d'un
client de portée étrangère : exit2, cause pipe absente conservée, propriétaire
initial toujours protégé et aucun contrôle adopté. Ce scénario ne rejoue pas
le conflit entre deux versions sur le PC et n'autorise aucun takeover de garde.


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

### P0-WIN-OWNER-IDLE — Ne pas abandonner une commande lors de l'arrêt du broker

L'ancien broker pouvait quitter sur une observation inactive alors qu'une
commande avait déjà commencé sa préparation ou attendait dans sa file.
Le correctifbb70dcf retient chaque admission jusqu'au traitement du job,
exige une nouvelle réconciliation après une admission concurrente et ferme
l'admission avant de quitter. Aucun mutex n'est retenu pendant le SDK ;
les délais et l'ownership natif restent inchangés, sans replay automatique.

**Qualification source :** contre-exemple concurrent déterministe sans SDK,
régression permanente et Clippy passés ; CI39217101436 exerce le vrai broker
zéro périphérique jusqu'à la libération explicite et sa disparition.
`proof_complete:true`, aucune erreur de cleanup. La correspondance avec la
cause unique du délai historique reste une inférence, pas une certification.
Ce correctif ne contourne pas le Busy d'ownership de P1-UPGRADE-ACTIVE.

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

**Qualification actualisée :** CI39217101436 passe le cycle complet depuis le
vrai tray : perte du seul helper possédé, reprise sans Allow, nouveau helper
authentifié, micro conservé puis libération explicite. `proof_complete:true`,
aucune erreur de cleanup ; consentement UAC humain et matériel restent non exercés.


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
ou la paire d'exécutables. État : **implémenté source0.20.0, qualification finale en cours**.

**Contrat :** identifier l'installation exacte de l'exécutable courant et sa
provenance : installateur Unix géré, Windows portable/Cargo, MSI, gestionnaire
Homebrew/AUR et `.app` macOS. Réutiliser les contrats de receipt/ownership
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
**État :** première consultation implémentée et qualifiée sur Intel/ARM, publiée dans **0.20.0**. La nouvelle capture utilisateur expose une présentation trop technique : refonte **0.21.0 non publiée** en cours, distincte de cette première livraison.
CI39579549269 et ses rapports app-smoke passent les quatre surfaces ; réouvertures
Finder ordinaires et fenêtre singleton conservées. Les limites matérielles et
d'autorisation du système restent explicites.

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

5. **Retour complémentaire utilisateur/ami pour0.21.0.** Une fenêtre essentiellement composée de diagnostics et de deux boutons ne suffit pas. Ajouter une **Vue d'ensemble** lisible avec statut micro/caméra séparé de la protection, accès observés actuellement et date de la dernière observation ; des **Réglages** réellement persistés (profil, notifications/son, historique local, pause et autostart) ; un espace **Diagnostics** copiable et distinct de l'accueil. Utiliser les composants AppKit natifs, le logo existant, les couleurs système clair/sombre, une hiérarchie visuelle et une mise en page utilisable autour de645×570 et au redimensionnement. Préserver l'onglet, le focus, la sélection et le défilement lors des mises à jour.
6. **Exploiter les diagnostics, pas seulement les déplacer.** Rechercher les causes des états dégradés/inconnus et corriger les bugs MCW démontrés, en distinguant les permissions système et le périmètre d'observation. Une inspection administrateur ponctuelle des profils n'est ni une permission TCC ni une preuve d'accès global à toutes les captures. Aucune collecte distante automatique, aucune élévation permanente, aucune suppression d'alerte pour afficher un faux statut rassurant.
7. **Parité Windows et Linux demandée explicitement.** Porter les mêmes vues et les réglages réellement applicables vers Win32 et GTK3, sans prétendre que les permissions/contrôles des trois systèmes sont identiques. Conserver la CLI, le tray discret et l'autostart silencieux ; `mcw tray open` ouvre/réouvre la fenêtre native existante ou démarre le frontend approprié. Sur Linux, l'absence de GTK3 ou de session graphique doit donner un prérequis explicite pour la GUI sans casser les commandes headless. Aucune nouvelle couche Python/Electron ni fenêtre factice.

**Validation :** vrais statut/fenêtre/menu/À propos sous macOS, compréhension des
états actif/inactif/inconnu/dégradé, ouverture/réouverture/fermeture sans doublon,
navigation clavier et lisibilité des sept langues. Aucun droit, autostart ou
contrôle de confidentialité ne doit être activé implicitement par ces surfaces.

## P1-TRAY-PROFILES — Sélection directe des profils sur tous les OS

**Demande utilisateur :** proposer les quatre profils directement dans les menus
natifs Windows, Linux et macOS, pas uniquement dans le menu AppKit.
**État :** macOS livré et qualifié depuis 0.18.0 ; Windows et Linux publiés
depuis0.19.0. CI39298930148 qualifie les choix natifs et la priorité de `--config`.
Les sept rendus Windows ont été relus, dont les choix désactivés sous `--config`.

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
| 2026-10-19 | P0-WIN-OWNER-IDLE — requête admise pendant la retraite du broker | **Correctif sourcebb70dcf, test ciblé et Clippy passés ; aucune hausse des délais** | L'ancien arrêt sur réconciliation inactive ne coordonnait pas la préparation/file des commandes. Ticket conservé jusqu'au traitement du job, admission fermée atomiquement avant retraite et réconciliation fraîche exigée après toute admission ; pas d'allocation par ticket, ni mutex retenu pendant le SDK. README actualisé | [INFERENCE] Cette course peut expliquer le délai de CI39213072436, mais sa cause unique n'est pas démontrée. Qualification du vrai service corrigé encore requise, pas de release publiée |
| 2026-10-19 | P0-WIN-OWNER-IDLE — contre-exemple concurrent sans SDK | **Exécutable jetable réussi : ancien prédicat Timeout, candidat réponse77** | Déclarations réelles du mécanisme candidat, canaux/threads privés et barrières imposant exactement le même ordre : intention annulée, job admis, observation inactive puis tentative de retraite. Ancienne décision abandonne le job pendant l'attente du transport ; candidat répond avant de fermer l'admission. Test permanent couvre aussi préparation, client parti et snapshot périmé | Aucun appareil, service installé, droit admin ou API SDK accédé ; ce smoke de concurrence ne remplace pas le scénario natif hébergé |
| 2026-10-19 | CI39215408250 — entrée native qualifiée, limites indépendantes conservées | **Déplacement et clic réels reçus, garde complète passée ; aucun popup observé** | Probe :move_count2/click_count1, client82/29 exact ; clic de la seule icône produit2 callbacks Shell, dernier514. Le manque général d'entrée ou de callbacks n'explique donc pas l'absence de popup dans ce run. Huit états restent refusés visuellement, aucun critère assoupli | Infobulle secondaire mise de côté. Ressources Windows :watch/idle quitte au warmup pour baseline et candidat, cause non établie ; qualification ressources macOS Intel aussi en échec. Linux et macOS ARM passent ; pas de succès global ni de MSI annoncé, aucune relance dédiée pour effacer ces échecs |
| 2026-10-19 | CI39217101436 — broker corrigé qualifié et limite visuelle isolée | **Vrais cycles gardes/reprise/libération et handoff/EOF passés ; trois jobs Unix verts** | Source3e28a17 : `guards.json` indique `proof_complete:true`, aucune erreur/cleanup, nouveau helper authentifié sans Allow et micro préservé ; phases complètes800/799 échantillons. Qualification fonctionnelle des ressources et comparaison baseline/candidat réussies ; budget RAM souple toujours non atteint | Windows ne reste rouge que sur les huit popups Explorer non observés ; MSI non exercé après ce refus. Pas de hausse des délais, pas de relance des échecs anciens pour les effacer, pas de modification de l'installation utilisateur ni de release publiée |
| 2026-10-19 | P1-TRAY-PROFILES — choix directs Windows/Linux | **Trois commits ciblés publiés en source, qualification native encore attendue** | a0dee5a Windows, fb4b4b2 Linux, 7929e25 CI : quatre choix localisés/cochés, commandes de cycle retirées, vraie persistance existante et garde `--config`. Windows applique le profil au collecteur conservé avant la première observation, sans frame de préférences périmées.39 tests frontend, Clippy/build et parsing/interop des pilotes passent | CI39238788001 : menus natifs inertes et stockage privé, pas de preuve matérielle ni d'exécution du collecteur. Revue : PID Linux publié atomiquement, messages interprocessus Windows bornés, sept langues Windows réellement prévues dans le pilote |
| 2026-10-19 | P1-TRAY-PROFILES — lecture d'un JSON en cours de publication | **Défaut du pilote reproduit hors UI puis corrigé dans d8ed19d** | Fonction réelle Wait-Record : fichier créé vide renvoyait null avant écriture, malgré absence de résultat complet. Smoke avec écrivain privé différé échoue avant/passe après attente d'un objet non nul ; fichiers/job jetables nettoyés, aucune API native ni entrée. Annulation du candidat39238451140 demandée, pas de relance aveugle | Qualification sur d8ed19d ; ne pas transformer une compilation ou un objet de fixture en preuve d'interaction native réussie |
| 2026-10-19 | CI39238788001 — profils Linux qualifiés, Windows arrêté avant UI | **Linux21 cas natifs passés, sept captures de reprise relues ; trois Unix verts** | Ordre arbitraire Meeting/Balanced/Development/Private, souris/clavier GTK, états DBusMenu, reprise du profil Private dans un nouveau processus et quatre commandes refusées sous config explicite ; menu russe désactivé relu. Windows garde/reprise/libération/ressources/EOF passent, mais PowerShell5 transforme le statut stderr Cargo « Finished » en NativeCommandError avant lancement du fixture | Infobulle Windows toujours en échec distinct ; aucun succès de profils Windows revendiqué sur ce run. Menus de fixture réels + handler/Settings de production, pas de matériel ni de parsing CLI natif certifié |
| 2026-10-19 | P1-TRAY-PROFILES — capture correcte du build natif Windows | **f05817e ; vrai Cargo no-run passé sous PowerShell5 avec stderr conservé** | ProcessStartInfo capture les deux flux, attend à deadline, journalise les deux et refuse tout exit non nul ; pas de changement à ErrorActionPreference. Smoke exécute la vraie fonction Build-ProfileTest, retrouve l'unique exécutable lib-test compilé et conserve le statut « Finished » sans NativeCommandError ; logs temporaires nettoyés, aucun test/UI/SDK lancé | CI39241828540 attendue pour les véritables interactions Windows ; ne pas annoncer une compilation comme validation visuelle |
| 2026-10-19 | CI39241828540 — première interaction Windows réelle | **Sélection souris et persistance Development passées ; preuve clavier arrêtée** | Build capturé correctement ; premier vrai sous-menu ouvert et choix Development appliqué par le handler/Settings de production, menu suivant le coche seul (`menu-1.json`), capture root relue. Le pilote Home/Right n'observe ensuite pas le sous-menu attendu ; il échoue sans fallback souris. Trois Unix et garde/reprise/libération/ressources/EOF Windows passent | Aucun succès global de profils Windows.201a0b6 remplace l'hypothèse Home par observation du vrai [MF_HILITE](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmenustate), navigation bornée, trace et refus si la cible n'est pas surlignée ; pas de sélection forcée ni de commande rejouée. CI39245550502 attendue |
| 2026-10-19 | CI39245550502 — interactions complètes Windows/Linux | **21 scénarios Windows et21 Linux passés ; trois Unix verts, infobulle seule encore rouge** | Windows : sept langues, souris/clavier alternés, quatre choix arbitraires, nouveau processus relisant Meeting, profil explicite désactivé/non coché et quatre commandes forgées refusées ;98 images, largeur native maximale373px. Trace réelle : Home ne surligne pas Profile, sept Down l'atteignent avant Right. Les cases/données natives passent | Relecture des PNG : plusieurs copies écran montrent le terminal sous-jacent plutôt que le popup, contrairement à es/ja/zh ; ces captures ne certifient pas les sept rendus. Ne pas clôturer le visuel sur le seul passed:true |
| 2026-10-19 | Test PC autorisé — capture du seul menu temporaire possédé | **Méthode PrintWindow validée localement, deux fenêtres privées fermées sans action** | Accord utilisateur explicite ; aucun MCW installé, profil, micro/caméra, autostart ou clipboard accédé/modifié. Premier WM_PRINT borné produit une image vide, méthode écartée ; PrintWindow HWND + RGB24 montre le vrai menu natif Balanced/Private/Meeting/Development avec Meeting coché. Deux processus propres sortent0, aucune entrée clavier/souris envoyée |7f2fa21 réutilise la capture native existante dans un worker séparé : HWND/menu visible, PID/image/naissance contrôlés, seul worker possédé tué à deadline15s pour borner PrintWindow synchrone. Parser/interop compilés ; CI39250411480 doit fournir les vrais rendus de profils, pas le menu local de sonde |
| 2026-10-19 | P1-TRAY-PROFILES — qualification finale native | **Terminé en source non publiée : Windows et Linux21 cas chacun** | CI39250411480 sur7f2fa21 : Windowsprofiles passe ;98 PNG PrintWindow/98 logs, stderr des workers vide, largeur maximale373px. Relecture des sept vrais sous-menus de restart : Meeting seul coché, quatre libellés lisibles ; root français explicite montre --config, sous-menu russe désactivé et sans coche. Données natives, souris/clavier, persistance nouveau processus et commandes explicites refusées restent qualifiées ; trois jobs Unix verts | Windowsglobal reste rouge uniquement sur l'ancienne preuve d'infobulle, MSI non exécuté. Ni parsing CLI, ni collecteur réel, ni matériel/protections ne sont certifiés par les fixtures inertes de profils ; aucune publication/tag ni installation locale |
| 2026-10-19 | TRAY-WIN — diagnostic local autorisé, sans contrôle produit | **Limite de survol conservée, aucune modification d'API Shell en production** | Première sonde : champ ctypes MSG.hWnd corrigé puis ciblage rect seul rejeté car renvoyait le chevron caché ; réutilisation des vrais Get-FixtureShellButton/Get-ShellWindows, nom + HWND/uID + rectangle et ajout temporaire du libellé français « Afficher les icônes cachées ». Exacte icône privée trouvée mais aucun popup, ancien protocole comme v4+NIF_SHOWTIP. Sonde fenêtre privée : aucun WM_MOUSEMOVE relatif/absolu observé malgré coordonnées déplacées ; WinSta0/Default, souris présente/visible, hover400ms, pas de capture/menu | Pas de cause générale d'injection affirmée : d'anciens runs hébergés prouvent WM_MOUSEMOVE reçu. Aucune preuve d'un bogue d'API corrigé, pas de bascule NIM_SETVERSION spéculative. Icônes et fenêtre privées supprimées avec sorties0 après corrections de sonde ; installé/SDK/clipboard/autostart inchangés |
| 2026-10-19 | Packaging Windows — MSI indépendant du survol Shell | **Qualifié sur runner jetable, source a0b1ff9 ; non publié** | CI39277258878/job118144756234 : vrai build/install/version/uninstall MSI per-user passé. Condition explicite : tests ordinaires et binaires release réussis, run non annulé ; le contrôle de popup conserve son échec et Windowsglobal reste rouge. Trois jobs Linux/macOS réussis | Aucune installation/désinstallation du MSI sur le PC utilisateur ; pas de qualification d'upgrade MSI inter-version. Prochaine version toujours non taguée/non publiée, source Cargo0.18.1 et stable0.18.1 inchangées |
| 2026-10-19 | CI39277258878 — clôture du lot profils/packaging | **Profils et MSI passés ; seul échec Windows : infobulle** | Nouvelle qualification Windows des profils passée, tests/Clippy/audit/build et scénarios natifs garde/reprise/ressources/EOF passés ; MSI réellement installé puis retiré, trois Unix verts. Les erreurs502 de gh run watch étaient du suivi API, pas des tests : observation bornée par endpoint du job et résumé final via jobs?per_page=10. Aucun scénario de produit relancé pour effacer un échec | TRAY-WIN reste non qualifié, P1-UPGRADE-ACTIVE reste distinct/non corrigé. Lot figé aux correctifs source et profils déjà faits, pas d'ajout de fonctionnalités ni de tag avant décision de livraison ; fenêtres/icônes privées fermées, observateur temporaire retiré |
| 2026-10-19 | Candidat0.19.0 — choix de livraison approuvé | **Préparation du paquet non publié, pas d'installation automatique** | L'utilisateur choisit « Paquet candidat non publié » après présentation des limites infobulle/update actif. Cargo et lock coordonnés0.19.0 ; garde de version du smoke Linux revue pour le même payload six membres, frontend/contrôle Linux inchangés hors profils. Notes candidates explicites ; metadata du site, pins publics, recettes, tags0.18.1 inchangés | Compiler/smoke réel de la version, workflow Release manuel sur branche sans Publish, puis contrôle des paquets/SHA-256 et livraison Windows. Latest API observé0.18.1/id415768652, aucun tag0.19.0 existant |
| 2026-10-19 | Candidat0.19.0 — smoke local de version | **Fmt, build release apparié et vraie CLI passés** | `cargo fmt --all -- --check`, `cargo build --release --locked --features windows-tray`, puis le nouvel exécutable répond exactement `mcw 0.19.0` ; script Linux revu analysé sans exécution privilégiée locale. Aucun tray/SDK/protection installé lancé ou modifié | Générer les archives signées par provenance CI du commit candidat exact ; l'affichage de version seul n'est pas une qualification complète ni une publication |
| 2026-10-19 | Candidat0.19.0 — vrais paquets construits, sans publication | **Release manuel39291306747 réussi sur les quatre OS, source6b9473c exacte** | Windows ZIP/MSI build et vrai package/install/uninstall passent ; Linux et deux macOS natifs/extraits passent, SBOM/sommes/provenance et bundle produits. Job publish réussi mais étape « Publish complete GitHub release » réellement skipped sur branche. Artifact verified-release-bundle/id12207186190,20834780 octets. CI39291264677 : trois Unix verts, Windows seul échec d'infobulle connu et approuvé pour candidat | Vérifier les neuf fichiers locaux, huit SHA-256 et attestations contraintes avant de livrer ; pas de nouvelle stable/tag, pas d'installation PC |
| 2026-10-19 | Candidat0.19.0 — vérification cryptographique et smoke du paquet | **Neuf fichiers livrés, huit SHA-256 et huit attestations valides** | Bundle du workflow manuel39291306747/source6b9473c : noms exacts et manifest vérifiés, provenance SLSAv1 contrainte au dépôt, release.yml, signer/source digest6b9473c, refs/heads/main et runners hébergés. Cinq vérifications initiales passent ; trois échouent à initialiser le client Sigstore, pas sur le digest. Racines officielles validées TUF générées une fois, seules ces trois vérifications reprises avec --custom-trusted-root ;8/8 finales sans relâcher les identités | Aucune qualification matérielle/UAC humain ajoutée. Infobulle non qualifiée et update propriétaire actif non corrigé restent les limites explicitement acceptées pour le candidat |
| 2026-10-19 | Candidat0.19.0 — livraison Windows sans installation | **Disponible dans Downloads/MicCamWatch-0.19.0-candidat-6b9473c** | ZIP : SHA256 `78d2d2b9c2cfecde763e91c8c78975384983deb2b34644f1fdc1fbac02a1af18`,5181971 octets ; MSI : `239415509f707feb670ecefca3f6dceb9f69326432bcb8148ab949b98e20d1f2`,4005888 octets. Archive quatre membres ; CLI extraite répond `mcw 0.19.0` sous chemins de préférences enfants privés ; MSI ProductVersion0.19.0 lu par MsiOpenDatabase READONLY. EXE, compagnon et MSI tous NotSigned ; fichiers extraits temporaires retirés | Aucun tray/installer/SDK installé lancé, aucune protection/clipboard/autostart modifié. Latest API après livraison reste v0.18.1/id415768652, aucun tag0.19.0. Le candidat n'est pas une cible de `mcw update`, ne pas forcer par MSI le refus d'ownership |
| 2026-10-19 | Retour candidat — profil courant visible au niveau parent | **Régression native reproduite avant correction ; correctif source en vérification** | Capture utilisateur : parent « Profil » sans sélection. Le test existant lit désormais les vrais libellés HMENU parent/enfants dans sept langues × quatre sélections × config explicite ou non : la légende racine doit contenir uniquement le profil effectif, sans texte localisé figé. Échec avant : false/true sur le profil sélectionné. Source compose directement un unique buffer UTF-16 « Profil — sélection », ou l'autorité --config | Tests/Clippy/build puis rendu natif et candidat actualisé à qualifier ; l'archive6b9473c déjà livrée n'est pas réécrite |
| 2026-10-19 | Retour candidat — ancien propriétaire micro encore vivant | **Conflit de versions confirmé par diagnostic utilisateur et processus, protections conservées** | Ancienne garde `mcw.exe __microphone-guard` PID19056 depuis .cargo/bin, candidat mcw-tray PID16868 ; enfant élevé PID20532 image candidate lisible par QUERY_LIMITED_INFORMATION malgré CIM path/command null. Hashes candidats cb483098… et565337dd… exacts. Intention manuelle requested:true, token automatique0 lue directement sans Store::open ni RPC ; aucune mutation native | UAC n'est pas une migration de garde. Pas de kill/release/unmute ni relecture du statut matériel pour rejouer le défaut rapporté ; handoff inter-version sûr reste P1-UPGRADE-ACTIVE, pas déclaré corrigé |
| 2026-10-19 | Profil parent Windows — correction locale qualifiée | **Avant : régression échoue ; après :39 tests frontend, Clippy et build release passent** | Sept langues et quatre profils + --config vérifiés via HMENU réel. Label racine expose la sélection effective, jamais la préférence inactive sous fichier explicite ; construction UTF-16 dans un seul buffer préalloué, pas de chaîne formatée intermédiaire. Aucun changement de protocole/version/ownership | Exercer les popups/captures du vrai menu sur runner isolé, puis livrer le candidat corrigé séparément ; ne pas substituer cette correction UI à un handoff des gardes du PC |
| 2026-10-19 | Profil parent Windows — rendu réel et candidat corrigé | **Terminé : natif21 cas Windows passé, candidat actualisé livré séparément** | Source62b550e, CI39298930148 : profil parent réellement rendu après persistance/restart ; captures françaises « Profil — Réunion », « Profil — Développement » et « Profil — Défini par --config » relues. Trois Unix verts, Windows conserve uniquement l'échec d'infobulle connu. Release manuel39298952381 quatre OS réussi, étape publication réellement skipped | Ancien dossier6b9473c et processus utilisateur inchangés par cette livraison ; pas de nouvelle instance tray démarrée automatiquement, migration des gardes non revendiquée |
| 2026-10-19 | Candidat62b550e — intégrité, provenance et version | **Huit sommes et huit attestations strictes valides, sans nouvelle initialisation des racines** | ZIP SHA256 `0b429dd7bfe9ec1b9072667a113907f4c7a7e81a01a521e3d03215769b350fcb`,5185304 octets ; MSI `9eb45ecc0c12a5f9a6f23b21d58cc401652b5e55b6dfe6d80d4c84328acd9580`,4009984 octets. Signature/source/ref/workflow/runners contraints ; CLI téléchargée `--version` retourne0.19.0 sous préférences enfants privées, MSI ProductVersion0.19.0 en lecture seule. EXE/MSI NotSigned, extraction temporaire supprimée | Disponible Downloads/MicCamWatch-0.19.0-candidat-62b550e ; aucun tag0.19.0, latest toujours0.18.1/id415768652, pas de swap/installation. L'archive ne réactive pas le micro et aucun tray candidat n'est exécuté par le smoke |
| 2026-10-19 | Demande urgente utilisateur — libérer immédiatement le micro | **Exécuté une fois avec le CLI compatible, résultat natif observé** | À la demande explicite « libère le moi », `C:\Users\stcui\.cargo\bin\mcw.exe unmute` exit0 : « Protection micro désactivée », « SDK: Le microphone est ACTIVÉ », endpoints1/hardware1. Pas de rejeu ni de kill de garde ; caméra non touchée. Tests suivants limités à la version d'un fichier extrait et aux archives, pas de reprise de protection | Ce démute autorisé n'est pas un handoff inter-version ni une preuve de capture physique. Ne pas réactiver implicitement la protection après le besoin urgent |
| 2026-10-19 | P1-UPGRADE-ACTIVE — combler la preuve manquante avant de modifier les verrous | **Audit source, pas de cause historique inventée** | Workflow public ancien : tray_started:false ; tests EOF = restart seul, tests readiness = fichiers octets/dormant, cycles vrais helpers = tray fermé avant release. Micro retire après réconciliation inactive/admission fermée ; caméra peut publier inactive avant de lâcher son lease au teardown, startup non armé30s/Allow terminé2s puis join. Polling passif ne démontre aucune rétention indéfinie | Ajouter le vrai tray actif aux preuves, ne pas introduire Quiesce/Allow/unmute forcé ni inférer une disponibilité depuis helper_active:false |
| 2026-10-19 | P1-UPGRADE-ACTIVE — transaction candidate native authentique | **Fixture ignorée hébergée implémentée, exécution attendue** | Ancien payload public0.18.1 attesté et candidat réel0.19.0, layouts upgrade et CLI candidat/ancien compagnon. Vrai lease global détenu sur autre thread : refus avant arrêt/écriture ; après libération du seul lease de fixture, vraie NativeOperations/apply, ancien child fermé coopérativement, paire/backups exacts et successeur authentifié, état privé inchangé. Préconditions hôte et custody/deadlines avant action | Ce n'est ni une acquisition publique du candidat ni l'appelant CLI installé mappé ; pas de versions falsifiées ni artefacts publics modifiés |
| 2026-10-19 | P1-UPGRADE-ACTIVE — release réelle avec frontend ouvert | **Scénario natif zéro-device étendu, pas exécuté localement** | Le contrôle Exit protégé reste conservé. Un frontend privé est ensuite rouvert avant Allow/unmute explicites sur runner vide ; attente des vrais handles de gardes sortis, vérification des mutex natifs disponibles et trois observations passives espacées avec tray encore vivant. Les mesures soutenues gardées restent séparées de cette phase inactive | Qualifier l'absence de réapparition des owners sur runner jetable ; ne jamais exécuter ce scénario sur le PC en réunion |
| 2026-10-19 | P1-UPGRADE-ACTIVE — public upgrade actif et limites EOF | **Mode opt-in active-tray et vérificateur commun implémentés** | Workflow public conserve mode sans tray par défaut ; mode actif utilise vrais anciens/current CLI/compagnons, upgrade/no-op et réparation indépendante du compagnon ancien. Exit du processus, bytes/prefs et successeur sont enregistrés avant assertion EOF ; un défaut legacy reste un échec explicite, sans fermer le successeur pour produire artificiellement EOF. Vérification publique extraite sans relâcher tag/source/manifest/provenance | Local :14 régressions transactionnelles, Clippy et YAML/Python/PowerShell/interop compilés passent ; unique erreur de compile du fixture corrigée en important WaitForInputIdle depuis System::Threading. Aucun produit/SDK/UI du PC exécuté |
| 2026-10-19 | P1-UPGRADE-ACTIVE — vrais anciens trays remplacés sans Exit manuel | **Preuve native passée**, sourcef65fc19/CI39306824376 | Deux layouts réels0181→019 et CLI019/compagnon0181 : ancien child vivant immédiatement avant apply, refus d'un lease global détenu sur autre thread préservant bytes/tray/état ; après libération du seul lease de fixture, vraie transaction passe, anciens PIDs8032/9992 sortis coopérativement, successeurs2764/9844 authentifiés et fermés proprement ; paire et backups exacts, préférences/état privé inchangés | Pas de mutation SDK/protection, pas de faux numéro ; fixture native n'est ni l'acquisition publique019 ni son CLI installé mappé. Aucun changement de l'algorithme d'ownership justifié par ces résultats |
| 2026-10-19 | P1-UPGRADE-ACTIVE — retraite réelle des helpers avec tray ouvert | **guards.json proof_complete:true, cleanup sans erreur** | Même CI : contrôle Exit protégé conservé puis nouvelle fenêtre privée active pendant Allow/unmute explicites sur hôte zéro périphérique. Tous les vrais handles de gardes sortis ; trois observations espacées conservent le tray vivant, les services inactifs et les deux mutex natifs disponibles. Reprise caméra sans Allow intermédiaire et micro préservé restent passés | Un booléen helper_active:false n'était pas utilisé comme disponibilité du verrou ; pas de contrôle matériel physique ni d'UAC humain, PC utilisateur/micro en réunion inchangés |
| 2026-10-19 | Mise à jour publique avec tray actif — diagnostic EOF distinct | **Deux cœurs de transaction réussis, workflow conservé en échec EOF** |39306851493 : vrais0180→0181 et CLI0181/ancien compagnon0180, sorties updater0 dans la borne, anciennes instances sorties, une instance suivante exacte, paire publique attestée et préférences préservées. À10s après exit, stdout_eof:false et stderr_eof:false dans les deux phases. No-op skipped après échec, aucune tentative rejouée ; nettoyage coopératif des seuls successeurs après enregistrement des preuves | Ce n'est pas un nouveau Busy dû au tray seul. Ancien EOF immuable déjà corrigé en source019 mais non rétroactivement ; ne pas publier une preuve globale verte ni forger une release pour tester l'acquisition du candidat. La cause exacte du refus historique protégé reste non attribuée |
| 2026-10-19 | Diagnostic micro — version incompatible ≠ autre namespace prouvé | **Corrigé3aff04f et refus natif qualifié** | CI39312572930 : guards.json proof_complete:true, refus unmute exit2 conservant open Windows control pipe/os error2 ; protection d'origine préservée, release explicite avec tray ouvert et vraie retraite des helpers passent, cleanup sans erreur. Local :14 régressions transactionnelles et Clippy passent. Aucun nouveau scénario permanent, seules les sorties des refus existants sont archivées | Trois Unix verts ; Windows échoue uniquement sur le popup Shell déjà non qualifié. Pas de bypass de version/ownership ni de replay du conflit sur le PC ; paquet62b550e livré antérieurement ne contient pas encore ce correctif |
| 2026-10-20 | Candidat019 actualisé avec diagnostic micro, sans publication | **Livré et vérifié**, source535bfc60d768976b98486c327fe7d1cca816980d | Workflow manuel39379984562 : Windows/Linux/macOS ARM/Intel et agrégation réussis, étape Publish GitHub release skipped. Neuf fichiers copiés à l'identique dans Downloads/MicCamWatch-0.19.0-candidat-535bfc6 ; huit hashes nommés égaux aux huit sujets de deux résultats de vérification du même ensemble signé sous contraintes repo/workflow/ref/source/signer exacts et deny-self-hosted. CLI ZIP exit0/mcw0.19.0 ; MSI ProductVersion0.19.0 ouvert read-only ; EXE/MSI NotSigned | Certains appels initiaux GH échouaient à initialiser Sigstore ; aucune signature ignorée, aucun appel réussi rejoué : les déclarations déjà vérifiées couvrent bien les huit sujets, tous comparés aux bytes. Preuve privée dans Temp/mcw-candidate-535bfc6-_vfc3mjy/evidence/delivery-verification.json. Anciens dossiers conservés, aucune installation/commande de protection/interaction tray ; tag019 absent et stable/latest0181 ID415768652 inchangée. Limites popup Shell et acquisition publique candidate conservées |
| 2026-10-20 | Retour utilisateur Windows après livraison du candidat | **Retour manuel positif consigné sans nouveau test** | L'utilisateur indique : « ça a l'air d'être bon. en tout cas pour windows ». Le tableau distingue désormais ce retour global de la qualification automatisée du popup ; aucune nouvelle interaction PC, installation, commande SDK ou boucle de tests pour le confirmer | Chemin/version effectivement exécutés et scénarios non détaillés ; ne pas inventer une validation spécifique de l'infobulle, de chaque contrôle matériel ou des autres OS. Candidat toujours non publié, décision de publication inchangée |
| 2026-10-20 | Autorisation explicite de publication stable019 | **Publication demandée, préparation du tag et des notes** | Après son retour Windows positif, l'utilisateur demande deux fois de publier et accepte de rapporter les bugs restants. Les preuves natives et les paquets candidats déjà obtenus sont conservés ; aucun nouvel échec produit bloquant découvert | Publier vrais artefacts depuis tag019 avec mêmes gardes de provenance et versions, puis vérifier la distribution publique. Aucune mise à jour automatique ou mutation de protection sur le PC utilisateur ; limites popup, vieux clients EOF et matériel non certifié conservées |
| 2026-10-20 | Publication stable019 et intégrité publique | **Publié/latest confirmé**, sourceac7a6e18306c7cf3944ec3e681aa204b68474e59 | Tag annoté036d204c8a2e1b0a95bf9019cb7f5366174e0fd6, releaseID418105233, release39383443795 quatre OS réussie ; neuf tailles/digests API et huit SHA manifestes vérifiés, déclaration signée couvrant les huit sujets sous ref tags019/source/signer exacts et deny-self-hosted. Vraie CLI ZIP exit0/mcw019, EXE/MSI NotSigned | Publication explicitement demandée, pas de rollback ni réécriture des018x. Matériel/UAC humain/popup non certifiés ; ancien invocateur garde son propre code |
| 2026-10-20 | Migration publique019 — compagnon actif et Unix | **Scénarios réellement passés, périmètre explicite** | Windows39385600826 : CLI019 courant mappé, compagnon0181 actif8756 remplacé par3976 exact, caller exit0/EOF stdout+stderr vrais, préférences/paire exactes ; no-op conserve3976 et les bytes, cleanup coopératif, aucune erreur. Legacy upgrade explicitement skipped. Unix39385600219 : installers publics0181→019 sur Linux/macOS ARM/Intel, bootstrap Mac et updater publics passent | Aucun frontend CLI mcw.exe tray dans cette qualification, ne pas généraliser au nouveau rapport PC. Aucune opération PC, SDK physique ou autostart utilisateur dans ces runners |
| 2026-10-20 | Pages019 / pins de distribution | **Publié HTTPS et relu dans le vrai navigateur**, commitcfc2d20 | Racine anglaise et /ru/ montrent019 et le bon ZIP public, captures relues et onglet fermé ; copyright exact. Quatre tests Node/export isolé passés ; huit accueils exportés contiennent Sourceac7/SHA installerb734 et vrais liens019. Recettes sourceac7/archiveSHA39a8d63d34cfbeae46b80c8720f7a0d99a2731bc6bfd83baebb61db4ba1b4941 et défaut workflow Unix019 alignés | Ancien site-dist utilisateur non touché ; anciennes captures171 non relabellées. Ni nouveau tap/AUR publié ni nouvelle qualification Homebrew/Arch déduite de la seule mise à jour des pins |
| 2026-10-20 | Retour PC après update019 — entrée CLI ignorée | **Diagnostic lecture seule, défaut de custody démontré** | Utilisateur rapporte encore l'échec tray actif et succès019 avec backup accès refusé. --version/statut passifs et hashes confirment paire019 ; micro non demandé/unmuted/garde inactive, caméra Allow sans helper au relevé. WMI indique mcw.exe tray PID22248 créé06:54:57Z ; HWND réel mêmePID et QueryFullProcessImageName limité pointe vers .mcw-update-m7Q2iL/previous-mcw.exe, SHA304ad22e… identique au CLI0181 vérifié. update() ne cible que le compagnon, contrairement à stop_installed_tray qui couvre les deux | Ancien frontend reste chargé malgré nouvelle paire ; explique le backup encore occupé, pas la cause exacte du Busy antérieur. Aucun update, stop, SDK, récupération/exécution/suppression du backup ni pointeur sur le PC. Correctif séparé en cours pour les deux hosts et contexte de lancement |
| 2026-10-20 | Correctif CLI-hosted0191 — implémentation et smoke local | **Sourcef120db7/92ee442, non publiée** | Deux images exactes reconnues ; host, arguments UTF-16, CWD et environnement conservés, namespace APPDATA/LOCALAPPDATA canonique exigé avant stop.24 régressions updater passent ; vrai snapshot natif du seul processus de test égale les API courantes argv/CWD/namespaces, Clippy propre. Frontal public CLI ajouté avec config relative, CWD différent et langue explicite, AST/C# compilés sans exécution locale | Capture mémoire bornée x64, aucun pointeur distant déréférencé localement. Pas de nouvelle allocation/copie du vecteur UTF-16 sur le chemin chaud du collecteur. Aucun update/stop/SDK PC ; version191 seulement dans la source |
| 2026-10-20 | CI39390170216 — premiers résultats et corrections des pilotes | **Deux transactions compagnon191 passent ; CLI arrêté avant transaction par mauvais critère idle** | JSON natif : old-pair et réparation current-CLI/old-companion passent avec contexte, refus/foreign/bytes/préférences conservés. Le pilote appelait WaitForInputIdle sur l'image console ; sonde du seul Python courant retourne WAIT_FAILED/1471. Critère remplacé par HWND/PID/image exacts, réponse WM_NULL et observation vivante2s, sans abaisser custody ni envoyer Stop pour tester readiness. Windows180 tests ordinaires passent, popup toujours non qualifié | macOS ARM : baseline immuable0161 échoue Quit sous burst puis cleanup PermissionError, aucun candidat mesuré ; Mac Intel/Linux passent, app/menu ARM indépendants passent. Cleanup macOS corrigé pour signaler seulement son enfant Popen non réapéré, pas tout le groupe ; l'échec baseline doit rester une ligne échouée. Nouveaux pilotes compilés/parsés et Clippy passent, preuve hébergée corrigée attendue |
| 2026-10-20 | Qualification CLI-hosted191, sourceb11f7d3 | **Cinq layouts natifs Windows passent**, artefact native-proof-windows-active-update, CI39394812502 | Compagnon0181 et réparation191/0181 ; CLI0181, réparation avec compagnon0181 et absent : sortie coopérative de l'ancien, successeur image exacte, arguments/CWD/environnement, paire et préférences conservés ; refus lease/foreign sans stop. Rollback natif CLI/context/state passe. Linux/macOS ARM/Intel réussis ; Windows180 tests lib, Clippy, gardes zéro-device et EOF passent | Job Windows global rouge uniquement sur qualification popup Explorer huit scénarios, limite non masquée. Baseline ressources Windows0161 watch/idle échoue, candidat fonctionnel passe mais comparaison/budget non qualifiés. Ni acquisition publique191 ni CLI installé mappé prouvés par ce pilote ; publication puis deux hosts publics à vérifier. Aucun SDK/update/arrêt sur le PC |
| 2026-10-20 | Construction release191, tag3dd70fc | **Tentative1 arrêtée par macOS Intel ; aucune release publique191 à ce stade** | Windows/Linux/macOS ARM construits. Intel :55 tests lib et version passent ; ressources candidat191 six profils passent, tray idle prêt et vivant20s puis stop CLI échoue2/EINVAL22 ; descendant_cleanup vide, completedfalse. Baseline0161 Quit/burst échoue distinctement. Log et JSON retenus, cause IPC/IO non établie | Uniquement jobs échoués relancés sur tag/source immuables ; aucun critère assoupli. Incident P1-MAC-TRAY-STOP conservé même si tentative suivante réussit. Site et pins191 préparés mais non poussés tant que vrais assets non publiés/vérifiés ; aucun contrôle PC |
| 2026-10-20 | Publication191 et vérification publique | **Stable/latest publiée09:46:23Z**, releaseID418238960, source3dd70fc, tag annoté1cc9141 | Release39397946877 tentative2 réussie, sans réécriture de tag/assets ni assouplissement des critères. Neuf tailles/digests API, huit SHA nommés et huit sujets signés égaux aux téléchargements réels ; gh attestation impose repo/workflow/signer/ref/source exacts et runners hébergés. ZIP exécuté seulement --version191/exit0 ; deux EXE et MSI NotSigned. Incident Intel ajouté aux notes publiques et laissé ouvert | Réparation/no-op publics lancés : CLI39419044223, compagnon39419044530 ; migrations Unix depuis01939419045377. Pins dépôt source3dd/archiveSHAb0447571ae5dfe037c6af083d1c97ebf7c6d9701c216da550f33eef57799763a, installerSHAb734 inchangé ; export isolé huit langues et quatre tests Node passent. Pages à relire après push ; ni tap/AUR externe ni installation PC |
| 2026-10-20 | Pages191 / contrôle des paquets Windows | **Déployé et relu dans le vrai navigateur** après commit89a7c8a | Racine html langen et /ru/ langru, huit liens de release191 exacts, copyright ©2026 Roman Cuisset ; captures visuelles relues, onglet dédié fermé. Captures historiques171 du produit non relabellées. MSI public ProductVersion191 lu par API en base read-only, handles fermés ; ZIP --version191 seul exécuté, aucun MSI installé | Anciens site-dist/installation/SDK/autostart et backup PC non touchés. Pins de recettes suivies alignés, aucune publication/qualification externe Homebrew/AUR déduite. Parcours publics Windows deux hosts et Unix attendus séparément |
| 2026-10-20 | Cible du workflow Unix automatique | **Fallback push/PR corrigé26703d9**, sans changement produit | Le défaut de dispatch191 avait été aligné, mais le fallback push restait019. Run39419234236 macOS échoue « public up-to-date call changed the installation » : le test installait019 puis demandait latest191 en l'appelant no-op. Log conservé ; fallback et dispatch visent maintenant191, YAML parsé et sélection vérifiée | Run explicite de migration19139419045377 non affecté ; aucun rerun de l'ancien défaut pour le confirmer, aucun critère supprimé ni installation PC. Nouveau push exerce le fallback correct |
| 2026-10-20 | Preuve finale publique191 — deux hosts Windows | **CLI39419044223 et compagnon39419044530 réussis**, JSON complets relus | CLI191 réellement installé/mappé et GUI CLI191 active avec compagnon019 : réparation acquiert vrais assets, ancien7488 sort, successeur3908 image exacte, configuration relative et languefr face au fichierde conservées et menu observé ; no-op garde3908/HWND. Compagnon019 actif5560→7284, même no-op. Paire égale au ZIP191 vérifié, préférences inchangées, stdout/stderr EOF avant cleanup alors que successeur vit ; cleanup coopératif des seuls owners retenus | Mode current-repair-only : aucun upgrade par un ancien invoker019 affirmé ; contexte de classification des politiques et contrôles matériels non exercés. Frontend PC0181 déjà renommé exige encore bascule explicite unique. Protections demandées/actives/restaurations et owners étrangers restent refusés ; aucun PC touché |
| 2026-10-20 | Preuve finale publique191 — Unix et clôture de livraison | **39419045377 trois OS réussis**, Linux/macOS ARM/Intel | Logs natifs relus : curl HTTPS réel, bash/zsh/fish fresh/réinstallation et migrations019→191 conservent PATH/préférences/historique ; receipt/ownership/rollback négatifs passent. Bootstrap et updater standalone publics macOS réussis, artefacts/logs retenus. Site191 et paquets/provenances vérifiés séparément | EXE/MSI NotSigned, app macOS ad-hoc non notarized, popup Explorer toujours non qualifié et incident Intel EINVAL22 ouvert. Ni publication/qualification externe tap/AUR ni hardware enforcement ni upgrade inter-version du bundle app inventés. Scripts observateurs et exécutables du seul smoke --version supprimés ; preuves/packages privés retenus, backup et installation PC intacts |
| 2026-10-20 | Source020 — lots parallèles approuvés | **Intégration en cours**, aucune release020 ni installation PC | CLI uninstall/worker/receipt et tray open raccordés avant Settings ; AppKitv3/fenêtre/About/menu, retrait protégé et logo Windows en source. Cargoversion020 et lock résolus sans mise à niveau des dépendances existantes hors nouveau compilateur de ressources build-only | Revues détectent débordement de frame UI, deadlock réservation caméra/polling et worker tardif : corrections demandées, pas de qualification annoncée. Smoke natif et commits de livraison après intégration |
| 2026-10-20 | Diagnostic exact EINVAL macOS | **Cause historique toujours ouverte** | Diagnostic1e71f8a/39423730151 échoue avant tout cycle : symboles C NOCANCEL mal liés. Labels Mach-O corrigés1415b43 ;39462504432 passe huit vrais cycles public0191 sur Intel et ARM, sans nouveau code source ni changement de contrôle | Un passage simple n'explique pas l'EINVAL sous sampler. Contexte ressources historique désormais instrumenté séparément, aucun retry/timeout augmenté ni faux « corrigé ». Artefacts natifs conservés, PC non touché |
| 2026-10-20 | Intégration locale020 — première vérification Windows | **181 tests passent ; Clippy propre et build debug réussi** | Une incompatibilité sha2.11 LowerHex et la déclaration NT dupliquée ont été corrigées en réutilisant l'API existante. Vrai binaire --version020, aide uninstall sept langues et producteur receipt avec hashes des deux exécutables passent ; Shell extrait le vrai logo PE et PNG relu. MSI debug construit par WiX, non installé | Ce n'est ni une release ni la preuve Start/Apps installés : qualification MSI/Shell jetable encore requise. Aucun uninstall, frontend, SDK, autostart ou registre utilisateur modifié sur PC ; logo macOS agrandi mais native à exercer |
| 2026-10-21 | Qualification source020 — résultats séparés | **Rendu AppKit et logo macOS passent Intel/ARM ; lifecycle IPC reste en échec** | 39568140011 : vraies captures du bouton, géométrie et consultation/About/menu passent indépendamment. Trace client : premier SO_RCVTIMEO réussit, second identique1s/0µs/16octets échoue EINVAL après réponse owner ; données brutes conservées | Fermeture du socket à expliquer, aucune augmentation/retry ni correctif proclamé. Linux Clippy arrondi corrigé9940377 ; preuve uninstall désormais persistanteb4e5cd0. Faux préalable MSI : getter COM Count null sous PS5 reproduit sans mutation, getter explicite Int32 vérifié et corrigé6758dd2 ; rendu MSI en qualification39569008606 |
| 2026-10-21 | Cause socket macOS et corrections de qualification | **Correctifs source intégrés, nouvelle qualification requise** | Listener O_NONBLOCK hérité côté BSD : acceptation suivie de lectures immédiatement EAGAIN et fermeture, puis EINVAL côté client. Passage explicite du seul flux accepté en bloquant ; pas de hausse du timeout. Projection UI de test isolée en sous-processus HOME/XDG pour ne plus créer l'état du login ; assertion de préservation maintenue. Texte AppKit autorisé à rétrécir, géométrie testée ; pilote Explorer utilise une vraie entrée SendInput et sortie coopérative | Refaire les parcours après changement. MSI branding39569008606 installé en contexte machine par argument vide ALLUSERS absorbant MSIINSTALLPERUSER : garder Scope=perUser du paquet sans override. Tests uniquement sur hôtes jetables, aucune installation ou protection PC touchée |
| 2026-10-21 | Audit TLS — correctifs ciblés | **Local vérifié, aucun ignore** | Échec CI Windows39572288019 : rustls-webpki0.103.15, roots/root-certs1.0.9 signalés par RUSTSEC-2026-0382/0381/0383. Lock corrigé vers0.103.16/1.0.10/1.0.10 seulement ;181 tests, Clippy et build Windows passent. Vrai transport self_update du build appelle en HTTPS l'API release et reçoit0191 ; source/exécutable de smoke supprimés, preuve conservée | Audit réussit, trois avertissements de maintenance zbus_names/zvariant/zvariant_derive restent visibles. Requalifier source native finale ; pas de modification des assets191 déjà publiés |
| 2026-10-21 | Fermeture BSD et intégration UI020 | **Huit cycles Intel/ARM et CI complète quatre OS passent** | 39579584237/source24303fe : démarrage/stop coopératif, trois réouvertures par cycle et kernel exit notes. 39579549269 : Windows/Linux/macOS ARM/Intel verts, vraies fenêtres AppKit/reflow/About et app Finder. Le premier succès open révélait seulement deux défauts du diagnostic : métadonnées sans errno et collecteurs éphémères confondus avec frontends persistants ; trames/exit notes désormais séparées | Désinstallation Linux passe39579584470 ; Mac exigeait à tort inventaire global sans reçu possédé, correction en source. SAFER garde TokenElevation=1 sur l'image Windows : aucun script lancé, passage à un vrai compte standard/desktop privé en qualification. Stable public191 inchangé |
| 2026-10-21 | Désinstallation020 — preuves Unix/Arch et contexte Windows | **Linux/macOS Intel/ARM et paquet Arch passent ; Windows reste à qualifier** | 39608664509/sourcec794 : retrait Unix/Cargo et refus corrompu/protection demandée préservés. 39609032385 : vrais makepkg/pacman install/reinstall/pkgrel-upgrade/remove et délégation sans I/O Internet ; socket IPv6 de capacité pacman seul n'est pas du trafic. Compte Windows Users réel, TokenElevation0, HKCU/profil et Explorer privé prouvés par39608664509/39608669894 ; scripts entrés, Cargo jobserver et mutex MSI refusent accès5 | Diagnostiquer ces prérequis natifs sans modifier politique, ACL ou privilèges ; worker/icônes non qualifiés tant que leur parcours n'est pas exercé. Aucun contrôle ou logiciel du PC touché |
| 2026-10-21 | Métadonnées Arch stable | **.SRCINFO0190 obsolète corrigé vers le PKGBUILD0191 existant** | Échec automatique39608506779 avant build : comparaison native makepkg révèle .SRCINFO périmé. Champs restaurés depuis son generated-srcinfo.stdout ; recette publique reste191/source3dd/SHA b044, candidat020 isolé sur branche de qualification | La publication020 ne remplace pas les assets191 et n'est pas annoncée avant qualification |
| 2026-10-21 | Qualification Windows — cause système et session ordinaire | **Cause établie ; micro-smoke de la session corrigée réussi** | 39612386830 : nouveau compte authentifié dans session2 étrangère, création de tous objets nommés refusée5 et accès CREATE_OBJECT au répertoire de session refusé ; objets anonymes passent. 39618500254 : authentification batch refusée1385. 39619305026 : seul SID jetable reçoit SeBatchLogonRight, vraie session0/profil/Explorer et TokenElevation0 établis, huit créations mutex/sémaphore anonymes/Local/Global passent. Aucun ACL de namespace ni compte existant modifié ; CI39608506806/sourcec794 verte quatre OS | Intégrer le même broker documenté au lanceur partagé, puis exercer les vrais MSI/icônes et worker/Cargo ; ce diagnostic n'est pas un PASS de ces fonctionnalités. Aucun changement sur le PC |
| 2026-10-21 | Séparation des surfaces Windows de qualification | **Lanceur ordinaire intégré passe ; parcours desktop encore en cours** | 39621404474 : vrai token0/SID/session0/profil/Explorer et huit objets système passent avec le lanceur Python réel. 39621628360 compile Cargo comme ce compte puis échoue à la readiness du tray ;39621644632 acquiert le mutex MSI mais le service refuse connexion80070005/1601 dans ce contexte batch | Branding revient au vrai bureau interactif du runner, sans demander de token ordinaire inutile pour lire des images : AssignmentType0 reste l'autorité de portée utilisateur, tous contrôles de payload/Start/Apps/PATH conservés. Timeout MSI ne tue plus le processus ni ne lance un nettoyage à résultat inconnu. Désinstallation : scénario de retrait différé sans frontend ajouté avant le cas tray ; aucun cas retiré, résultat global reste en échec si un cas manque. Dispatch ciblé évite de reconstruire Unix lors d'un changement Windows seul |
| 2026-10-21 | Homebrew — contrat explicite approuvé par l'utilisateur | **Source adaptée, six cas natifs conservés** | Homebrew6.0.22 remplace HOME/XDG dans post_install et ne permet pas l'écriture des LaunchAgents/receipts personnels. Utilisateur choisit « Commande de rafraîchissement ». Hook inefficace retiré ; upgrade sans nettoyage puis mcw autostart refresh puis cleanup, chaînés avec && pour ne jamais nettoyer après un refus. Les baselines0161 et intentions chargé/dormant/désactivé restent exercées | Requalifier Intel/ARM ; pas de réussite limitée au HOME temporaire ni de migration automatique annoncée. Pins publics191 inchangés |
| 2026-10-21 | Windows — preuves consumer plutôt qu'hypothèses de rendu | **Retrait sans frontend prouvé ; problème desktop identifié** | 39625456368 retire vraiment la paire portable après sortie CLI avec données intactes. 39654297585 : console « failed to register system tray icon » dans session batch ; pas d'exception supprimée. Shell du vrai PE correspond au logo ; SHDefExtractIcon choisit un rendu différent selon conteneur ICO/PE. MSI conserve toutes entrées PATH brutes, type ExpandString et ordre, retire seulement le séparateur final vide lors du retrait | Supprimer les assertions incidentelles sans relâcher les cas de protection/ownership ; compléter toutes surfaces réelles et trouver un contexte de bureau ordinaire pris en charge. Aucun contrôle du PC touché |
| 2026-10-21 | Logo Windows — clôture de qualification native | **36 surfaces/taille passent, images réelles relues** | 39655798674 réutilise MSI020 exact SHA F37927EA… et sa paire installée, vérifie AssignmentType0, sources et retrait réels ; captures Shell CLI/tray/Démarrer et extraction de l'icône Apps16–256 correspondent au canonique. PATH conserve entrées non vides brutes, type et ordre ; seul séparateur final vide normalisé par Windows Installer | Lot logo terminé ; prochain travail Windows concerne l'environnement ordinaire interactif pour le retrait avec frontend, pas l'image de marque. Version020 toujours non publiée |
| 2026-10-22 | Homebrew — première matrice explicite et correction du vrai parseur | **ARM chargé et dormant qualifiés ; défaut désactivé corrigé en source** | 39655559884/source2bda : deux rapports qualified=true incluent upgrade0161→020, refresh ordinaire, nettoyage/test/retrait/réinstallation et état utilisateur conservé. ARM désactivé renvoie réellement « => disabled » : pilote et produit ne reconnaissaient que les booléens. Parseur Rust accepte disabled/enabled et true/false, garde le refus des valeurs inconnues et le label exact ; smoke de la vraie réponse et trois régressions isolées passent sur Windows, sans launchctl ni contrôle local | Requalifier les six cas natifs. Intel bloque dans la formule amont Z3 avant Rust, pas dans MicCamWatch ; réparation exacte du seul helper Python déclarée dans les preuves, vrais builds et tests conservés. Prototype bureau Windows39657367246 atteint le délai sans premier rapport ; diagnostic par étapes et capture réelle lancé, pas de succès déduit |
| 2026-10-22 | Homebrew — trois intentions ARM qualifiées, provenance Intel clarifiée | **39768723173/source201e : ARM chargé, dormant et désactivé passent** | Rapports qualified=true relus sur les trois parcours complets ; le vrai refresh conserve l'override natif disabled. Le checkout core fourni par Intel contient déjà Z3 5.1.0/python3.14 correct, contrairement à la formule API5.3.1 : le garde de hash refuse justement une réparation inapplicable. Patch de formule supprimé ; graphe officiel source cohérent, non modifié, API désactivée, commit/origine/recette/versions et vrais tests Z3 conservés | Relancer seulement les trois cas Intel après ce changement de provisionnement, sans refaire les trois preuves ARM inchangées. Matrice complète de six reste le défaut. Windows : consentement du certificat exact réussi dans VM, puis faux refus du pilote dû à ExitCode=null sans handle conservé ; reproduit avec enfant inoffensif et corrigé dans le seul prototype |
| 2026-10-22 | Qualification intégrée après correction launchctl | **CI39768698678/source201e verte sur Windows, Linux, macOS Intel et ARM** | Quatre jobs terminés success. Rapports AppKit des deux architectures relus : native_menu_exit, singleton consultation fermer/réouvrir, Finder ordinaire, doublon préservant le propriétaire, readiness et bundle déplacé tous passed. Contrôles appareils, capture physique, autorisations OS et notarisation restent explicitement non testés/non accordés | Cette CI ne remplace pas le retrait Windows avec frontend dans un vrai compte ordinaire ni les trois parcours Homebrew Intel encore en qualification |
| 2026-10-22 | Windows — prérequis de bureau ordinaire identifié hors application | **Blocage de qualification, pas de PASS inventé** | RDP strict avec certificat local exact vérifié : authenticité réussie1149, Users+Remote Desktop Users seulement, SeRemoteInteractiveLogonRight déjà accordé à555. Le nouveau bureau s'arrête avant le bootstrap ; Server2022 signale Winlogon4005 « unexpectedly terminated », Server2025 échoue à CsrInitialized/0x80004005. Rendu logiciel documenté et profondeur32 ne changent pas le résultat. Aucun mcw du fixture exécuté dans ce chemin, aucune validation TLS/UAC ni ACL contournée ; certificat ajouté seulement dans VM jetable, jusqu'à destruction | Fournir un véritable bureau Windows ordinaire jetable fonctionnel pour terminer les deux hosts actifs, autostart, lock/refus et délégations de retrait. Ni session batch ni compte admin ne remplacent ce critère. PC utilisateur, SDK, autostart et MSI installés non touchés |
| 2026-10-22 | Intel — simplification du prérequis Homebrew | **923e21e en qualification native39774103433** | Le checkout fourni comporte une modification rustup étrangère au besoin : conservée, pas réinitialisée. Les contraintes de reconstruction LLVM/Rust et de propreté globale étaient étrangères à l'acceptation du paquet MCW ; supprimées. Homebrew choisit normalement ses bouteilles vérifiées ou sources depuis un checkout cohérent, sans mélange API ; commit/origine/delta, version installée et vrai host rustc consignés. L'application reste réellement construite de source | Attendre et lire les trois rapports Intel, sans déclarer le gestionnaire qualifié depuis la seule syntaxe. Trois parcours ARM201e et CI quatre OS201e restent acquis ; stable191 inchangée |
| 2026-10-22 | Homebrew — clôture des six états natifs | **ARM39768723173 et Intel39774103433 réussis**, même source produit201e | Six rapports qualified=true relus : chargé/dormant/désactivé, véritable install0161→upgrade020, refresh ordinaire explicite avant nettoyage, no-op sans réactivation, menu AppKit, test/retrait/réinstallation et données conservées. Intel utilise Rust Homebrew1.98.0 natif x86_64-apple-darwin, checkout corea18de1a et delta fourni conservé ; ni patch de formule ni reconstruction artificielle de compilateur. Qualification externe/tap publié non revendiqués | Lot Homebrew terminé. Publication020 et clôture générale restent suspendues au véritable retrait Windows avec deux hosts actifs et autostart sur bureau standard jetable fonctionnel. Stable191 et pins publics non modifiés |
| 2026-10-22 | Clôture atteignable et prérequis externe restant | **Neuf lots terminés, qualification Windows active non clôturée** | Changements et preuves Homebrew intégrés jusqu'à38085a0 ; worktrees temporaires Windows/Arch retirés sans force, branches de qualification et artefacts conservés pour reprise. Aucun hôte SSH isolé configuré. Les bureaux frais des runners2025/2022 ont échoué avant le bootstrap malgré authentification/droits corrects ; ne pas relancer le même scénario pour confirmer cet échec | Reprise : disposer d'un Windows jetable avec vrai bureau utilisateur ordinaire fonctionnel, puis exercer deux hosts actifs, autostart, contention/refus, MSI/Cargo et données conservées. Finir alors la qualification/livraison020 ; ni marquer le lot terminé ni publier une preuve partielle. Site/recettes publics restent191, huit brouillons site préexistants conservés |
| 2026-10-22 | Test réel PC — retrait Windows actif et restauration utilisateur | **Deux parcours natifs passent en17,83s**, sans changement produit | Utilisateur demande le test sur son PC puis choisit « Test complet avec restauration ». Compte réel non administrateur (TokenElevation0), copies020 exactes du payload natif vérifié402cd89b…/fad36109…. Compagnon et CLI atteignent readiness, commandes uninstall acceptées suivies des vrais result.json removed/data_preserved=true ; processus sortent0, deux EXE/receipt retirés, données de test conservées et autostart de test absent.191 réel relancé après restauration de l'entrée Run exacte, type/StartupApproved conservés, argv/CWD/environnement capturés en mémoire ; hashes de la paire installée identiques, micro unmuted/caméra allowed. Preuves locales : mcw-local-removal-ndqudaxb/proof.json et deux résultats workers | Prérequis de bureau pour ces deux parcours levé. Ne pas présenter ce smoke comme un test de capture physique, de garde activée, de refus sous contention ou de délégation/retrait MSI/Cargo Windows. Aucune désinstallation de la vraie installation, aucune commande de sourdine/blocage, aucune conservation de l'environnement utilisateur dans les artefacts |
| 2026-10-22 | Windows Cargo — véritable installation et retrait isolés | **Parcours gestionnaire passé**, source020 courante | cargo install réel, root temporaire mcw-cargo-uninstall, deux exécutables produits en40s. Une vraie receipt portable valide ajoutée ne prend pas priorité sur .crates2.json : mcw uninstall retourne2 et la commande Cargo avec root exact, sans modifier binaire, métadonnées ni données. Puis cargo uninstall réel retire ce seul paquet et conserve les données. Rapport local mcw-final-qualification-r0xj33zk/cargo-proof/result.json passed | Cargo Windows qualifié. Refus natifs en cours ; MSI doit rester sur VM jetable car l'inventaire non mutatif du PC révèle déjà deux produits0190 et un raccourci existants. Aucune installation/désinstallation MSI sur le PC, ni démarrage/contrôle matériel changé |
| 2026-10-22 | Windows — six refus réels sans mutation du PC | **Six cas passed**, vrai compte ordinaire et tray191 conservé | Contention déterministe de la vraie installation avant première instruction du worker, frontend étranger, compagnon altéré, receipt de version inconnue, cible hardlinkée et préférence de protection demandée : chacun refuse pour sa cause réelle, garde payload/données byte-identiques, GUI191/PID/image/naissance et registres/autostart identiques avant/après. Preuve locale local-refusals-ej2ag4fv/proof.json. Le pilote traite désormais le vrai conhost Windows comme enfant système authentifié (chemin System32 natif + parent kernel), jamais comme worker ni enfant arbitraire ; tous les processus de test doivent finir avant libération du verrou | Refus et Cargo Windows qualifiés, aucun mute/unmute/block/allow ni activation de frontend/protection pendant ces six cas. MSI en qualification isolée ; aucune modification des MSI existants du PC |
| 2026-10-22 | MSI Windows — qualification native du gestionnaire clôturée | **Vrai compte ordinaire, installation/délégation/retrait passent en11,91s** | Paquet réel WiX5.0.2 perUser, famille/composants/raccourci/AppModel propres au test, paire020 exacte402cd89b…/fad36109…. AssignmentType0 et InstallLocation avec espaces observés ; mcw uninstall retourne2 avec msiexec.exe /x au ProductCode00064190… réel, sans mutation du payload, des données, du raccourci, de PATH ni de la registration. Retrait natif effectif du produit/deux EXE/raccourci/clé de test, trois fichiers utilisateur conservés. Rapport host-msi-private-family/evidence-native-quoting/result.json passed ; deux MSI0190, tray191/PID5632 et contexte utilisateur originaux identiques | Qualification du routage MSI terminée, pas une migration du MSI de publication inchangé sur ce PC. Le MSI de marque exact était séparément qualifié installé/retiré par39655798674. Aucun mute/unmute/block/allow ni arrêt du tray réel ; publication020 et vérification des futurs assets restent à exécuter |
| 2026-10-22 | Pilote MSI — erreur de guillemets établie et corrigée | **Pilote permanent réellement exercé avec succès** | La liste argv Python citait toute l'affectation INSTALLFOLDER avec espaces ; le parseur propre à MSI affichait son aide même sous /qn. Surface Windows Installer observée, fermeture normale WM_CLOSE et sortie1639, aucun kill ni installation prétendue. capture accepte désormais une ligne native Windows explicite et la conserve dans les traces ; msi_case cite NAME=\"valeur\" et utilise System32 pour installation/retrait. Le vrai msi_case corrigé passe avec le paquet isolé, données conservées et tray/paire/autostart inchangés ; permanent-driver-smoke/result.json passed | Pas de changement du consommateur Rust, ni de suppression d'erreur ou réduction de chemin pour contourner les espaces. Les refus préalables de contexte et la faute de syntaxe temporaire restent des échecs de qualification, non des défauts prouvés de MicCamWatch |
| 2026-10-22 | PC utilisateur — périmètre et état restauré | **Accord explicite aux tests directs, anciens produits préservés** | Le compte standard temporaire a atteint son vrai profil/TokenElevation0, mais son namespace local refuse un mutex : arrêt avant tout MSI, suppression native du profil après déchargement puis du seul compte/répertoire créés. Les deux anciens produits proviennent des candidats6b9473c et535bfc6, tous deux0190/perUser : leurs tables Upgrade excluent la version égale, d'où deux registrations possibles, pas la preuve de deux processus actifs. Aucun de ces deux produits retiré ; le test final utilise la vraie session ordinaire existante avec identités et données de test séparées | Pas de nettoyage implicite des anciennes installations, pas de droits ajoutés au compte habituel, pas de changement d'ACL/politique/service existants ni de protection appareil ; traces privées conservées |
| 2026-10-22 | Publication 0.20.0 — clôture et vérification publique | **Version 0.20.0 publiée stable/latest**, run 39807585260 | Tag annoté 547f6185… sur source a2652202…, releaseID 420936561. Neuf assets publics téléchargés et relus, huit sommes SHA256SUMS vérifiées et huit attestations de provenance SLSA v1 validées par gh attestation verify. Site et recettes distribution alignés ; aucun asset réécrit, aucune fausse signature revendiquée | Version 0.20.0 livrée et clôturée |
| 2026-10-23 | Urgence — politique de confidentialité du site / SmartScreen | **Brouillon intégré et vérifié localement, aucune publication autorisée** ; branche isolée work/privacy-policy-site, code1cef2da | Audit source020 des données locales/réseau et documents officiels Cloudflare/GitHub ; historique activé par défaut, rotation sans échéance par âge, exception réseau uninstall portable sans reçu et fichiers temporaires macOS explicités. Site public : beacon Cloudflare injecté mais réellement bloqué par CSP, NEL annoncé ; aucune absence générale de traitements prestataires prétendue. Quatre tests Node passent ; export version020, huit routes privacy ×1365/390/320=24 surfaces200 sans débordement,16 accueils/404 avec liens corrects, navigation réelle et contenu sans JS exercés ; deux revues sans défaut concret | URL cible https://miccamwatch.pages.dev/privacy après accord/déploiement, non annoncée comme déjà publique. Confirmer identité/rôle du responsable, contact privé dans site/privacy.json, bases légales, paramètres/garanties hébergeurs et conservation des échanges privés ; actualiser les formulations de brouillon avant publication, puis vérifier HTTPS réel. Aucun push, déploiement, tag, release ni modification des contrôles/app ; travail GUI antérieur conservé et suspendu hors de cette branche |
| 2026-10-23 | Confidentialité — finalisation et publication autorisées | **Accord explicite reçu**, Roman Cuisset développeur indépendant sans société enregistrée ; contact officiel miccamwatch@gmail.com confirmé | Sept langues finalisées ; mentions provisoires et branche de rendu sans contact supprimées. Gmail ajouté aux destinataires et sources officielles, critères de conservation sans délai inventé, bases légales et droits cohérents. Quatre tests Node et export020 passent ; vrai runtime Wrangler Pages4.152.0 :24 surfaces privacy sans JS à1365/390/320px,16 accueils/404, sitemap16URL, robots200, mailto activé sans envoi, vrais clics footer/langue et onglet Linux passent. Deux revues indépendantes sans défaut bloquant. Diff public main→travail limité à16 fichiers site/PRIVACY/roadmap ; src/Cargo/installateurs/distribution/workflows inchangés | Push privacy-only autorisé vers main par avance rapide depuis1c19ae5 ; déploiement Git Cloudflare établi par le check du précédent main. Vérifier ensuite les vraies URL HTTPS, toutes les langues, contact sans JS, SEO/robots et affichages ; aucun résultat public encore revendiqué à cette étape. Travaux GUI ae5457d préservés hors de cette branche, aucune release application |
| 2026-10-23 | Confidentialité — publication et qualification publiques clôturées | **Politique publique opérationnelle**, main69f51c9 ; commits privacy1cef2da/d868861/69f51c9, aucune fusion GUI ni release application | Cloudflare Pages f8417b3e-b6ed-4c2f-bcf5-92df86f9fea0 success, CI site39916536804 success. HTTPS /privacy et /en,/fr,/de,/es,/ja,/zh,/ru/privacy :24 surfaces200 sans JS sur1365/390/320px ;16 accueils/404 et liens footer corrects, sitemap16URL, robots200, canonicals/hreflang/OG cohérents. Mailto miccamwatch@gmail.com visible et activable sans envoi ; navigation réelle footer/langue et onglet Linux passent. Alias .html et slash final redirigés308 vers les URL stables ; politiques Cloudflare/GitHub/Google200. Beacon injecté effectivement bloqué CSP, NEL annoncé, cookies/localStorage/sessionStorage vides dans le contrôle. Install.sh public51249octets, SHA256b7348c9a…384d1bb identique au manifeste020, non exécuté | URL anglaise officielle communicable à Microsoft : https://miccamwatch.pages.dev/privacy ; française https://miccamwatch.pages.dev/fr/privacy. Consultation réelle par Microsoft, décision SmartScreen et indexation effective non prétendues. Conserver la revue administrative des paramètres/accords prestataires et critères de conservation ; mettre la notice à jour si les traitements évoluent. Preuves privées mcw-privacy-publication-proof-20261023 ; travaux GUI ae5457d et installateurs préservés |
| 2026-10-23 | SEO Google — nom de site MicCamWatch et données structurées WebSite | **Données WebSite JSON-LD, canonical racine et favicon PNG intégrés** ; branche dédiée work/site-name-seo depuis main35f15f9 | Cause : absence de balisage WebSite sur le sous-domaine pages.dev menant Google à déduire le nom d'hébergeur « Cloudflare ». Ajout de l'objet Schema.org WebSite (name: "MicCamWatch", alternateName: "MCW", url: "https://miccamwatch.pages.dev/") sur les huit accueils dans le HTML statique sans JS ; og:site_name MicCamWatch et titre officiel préservés ; canonical/hreflang/sitemap alignés sur / pour l'anglais et /fr/, /de/, etc. pour les versions localisées (/en/ conservé sans boucle de redirection) ; favicon officiel PNG 96x96 généré et déclaré aux côtés du SVG pour conformité Google Search. Tests localisation réussis (5/5), export statique conforme, pages de confidentialité et installateur byte-identiques | Publication autorisée vers main ; déploiement Cloudflare Pages et vérifications publiques HTTPS post-déploiement. Travaux GUI de l'application préservés |
| 2026-10-23 | Visibilité GitHub & Google — présentation dépôt, README et maillage | **Description, topics, README enrichi et liens GitHub intégrés** ; branche dédiée work/github-visibility depuis main7825c97 | Métadonnées GitHub actualisées via gh repo edit : description alignée (« MicCamWatch — Free, open-source microphone and camera activity monitoring for Windows, Linux and macOS. »), topics enrichis avec miccamwatch et open-source (17 topics au total). README réorganisé en tête avec titre explicite, badges d'état et site officiel, liens directs (site, releases, politique), captures réelles top/tray, tableau récapitulatif des plateformes et guide de démarrage rapide, sans suppression des sections techniques existantes. Liens directs vers le dépôt GitHub ajoutés au footer de l'accueil et de la page de confidentialité. 5 tests unitaires réussis, export statique vérifié | Déploiement Cloudflare Pages et vérifications publiques post-push. Travaux GUI préservés |
| 2026-10-22 | P1-ARM — implémentation native et qualification engagée | **Source0.21.0 non publiée**, branche work/linux-arm64-021 | Commits292e850/9ff3471/e6ea9a1 : version mineure, sélection Linuxaarch64, ELF183/62 et deux binaires validés avant première sonde ; migrations de format contrôlées, refus d'en-têtes corrompus, runner ubuntu-22.04-arm et preuves ABI/glibc/bindgen, ressources candidates sans faux baseline ARM. Syntaxes shell/Python/YAML passent ; deux revues statiques n'identifient pas de défaut concret, sans remplacer un runtime | CI39828353569 et retrait natif39828453493 sur source exacte e6ea9a19b4195a3bbb91e3ed0679dba891f3a6bd en cours. Lire les résultats/artefacts avant qualification ; aucune publication021, aucune mutation de l'installation/protection PC ; Raspberry Pi/SBC physique non exercé |
| 2026-10-22 | P1-ARM — premiers résultats et comparaison historique absente | **Qualification en cours, pas de publication** | Retrait natif39828453493 réussi sur UbuntuARM/x64 et macOSARM/Intel, vraies installations gérées/Cargo, données conservées et refus de retrait dangereux. CI39828353569 : Windows/Linuxx64/macOSARM/Intel réussis ; ARM compile/clippy/tests/SPA-C, contrôles ELF et frontière root réussis, puis arrêt du scénario PipeWire avant ses contrôles candidats car il exigeait une archive ARM0161 inexistante | native_pipewire_limits.py réutilise désormais la déclaration explicite MISSING_BASELINES : comparaison historique indisponible sur ARM, aucun remplacementx64, tous les scénarios candidats maintenus. Nouvelle qualification native nécessaire ; le premier run ARM reste échoué et n'est pas présenté comme validé |
| 2026-10-22 | P1-ARM — qualification logicielle native clôturée | **Source2998946 qualifiée,0.21.0 non publiée** | CI39832624207 réussit Windows/Linuxx64/LinuxARM/macOSIntel/macOSARM. Artefacts ARM relus : uname/rustc/cc natifs, glibc2.35 et headers PipeWire/SPA0.3.48ARM ; deux ELF183 et bindings conservés ; installation réelle,14 refus ELF par installateur, format1 contrôlé, root read-only/rollback, PTY, six pannes PipeWire/reprise sans faux STOP et capture virtuelle conservée, vrai panel/menu/autostart. Retrait géré/Cargo39828453493 quatre Unix réussi, données conservées | Pas de publication/archive ARM historique/Pi physique ni garantie15Mo. Les nouvelles GUI trois OS demandées pendant ce lot sont distinctes et restent à qualifier sur leur source finale ; aucun contrôle de confidentialité PC changé |
| 2026-10-23 | Sauvegarde WIP — GUI trois OS et préparation ARM64 | **Travaux préservés, 0.21.0 non compilée et non publiée**, commit WIP sur work/linux-arm64-021 | Audit de reprise par inspection : 12 fichiers suivis modifiés (+1765/−497) et 2 modules GUI non suivis (windows_desktop 361 l., linux_desktop 526 l.). Aucune compilation ni exécution GUI depuis le début du chantier (0 invocation cargo/swiftc dans les journaux des quatre agents). Deux références Windows orphelines empêchent la compilation : `DesktopWindow::is_visible` (tray.rs:1054) et `show_diagnostic_window`/`DETAILS_EDIT` supprimés mais encore appelés (tray.rs:3718/3720). ARM64 : CI39832624207 et retrait39828453493 réussis sur source2998946 ; aucun matériel ARM réel, aucune publication | Reprendre par les deux correctifs de compilation puis `cargo check` par cible ; détail exhaustif dans la section « 2026-10-23 — GUI Windows/Linux/macOS et Linux ARM64 — État de reprise » ci-dessous. Aucun merge dans main, aucun tag, release ni déploiement |

## 2026-10-23 — GUI Windows/Linux/macOS et Linux ARM64 — État de reprise

**Statut : travail sauvegardé et documenté. Les nouvelles interfaces GUI n'ont été ni compilées, ni lancées, ni validées. Aucune version 0.21.0 n'est publiée et la branche n'est pas fusionnée dans `main`.**

### Origine et chronologie vérifiées

| Moment | Fait établi | Source |
| --- | --- | --- |
| 2026-10-22 19:15 | Retour de l'utilisateur (capture macOS 645×570, mur de diagnostics) : réglages, statut compréhensible, présentation plus soignée | transcription de session |
| 2026-10-22 19:20 | Contrats d'implémentation du protocole desktop v4 (macOS), puis extension Windows/Linux | `local://mcw-021-macos-gui-contract.md`, `local://mcw-021-windows-linux-gui-contract.md` |
| 2026-10-22 19:35 | Agents `LinuxFullGui` et `WindowsFullGui` lancés en parallèle de `MacGuiAppKit` | transcription |
| 2026-10-22 19:44:04 | Les agents Linux et Windows s'arrêtent à 70 ms d'intervalle **sans résumé** : travail interrompu en cours d'écriture | fichiers `.md` vides, derniers enregistrements 19:44:04.332/.401 |
| 2026-10-23 05:38 | Reprise du parent seul (« continue »), orientée correctifs transverses | transcription |
| 2026-10-23 05:46:41 | Dernière modification de code (`src/watcher/unix.rs`) | journal d'outils |
| 2026-10-23 05:51:08 | Arrêt d'urgence demandé pour la politique de confidentialité | transcription |

### A. Objectifs des deux chantiers

- **GUI 0.21.0** : remplacer le mur de diagnostics macOS par une véritable application native retenue, avec trois espaces **Vue d'ensemble / Réglages / Diagnostics**, puis porter la même structure sous **Windows (Win32)** et **Linux (GTK3)**, sans dégrader la CLI, le tray discret, l'autostart silencieux ni les commandes headless.
- **Linux ARM64** : produire `aarch64-unknown-linux-gnu` et `miccamwatch-linux-aarch64.tar.gz` au même niveau de baseline que x86_64 (glibc 2.35, PipeWire/SPA 0.3.48), avec contrôles ELF stricts et qualification native hébergée.

### B. Fonctionnalités déjà développées

| Plateforme | Fichiers | Contenu développé |
| --- | --- | --- |
| macOS | `native/macos_desktop.swift` (+563), `installer/tests/native_macos_menu.py` (375 l. modifiées), `installer/macos/app_smoke.swift` | Fenêtre AppKit retenue à trois onglets, marque et version, cartes clair/sombre, sept langues, protocole v4 strict (bornes, enums, horodatage UTC), actions `enable`/`disable` idempotentes, conservation onglet/focus/défilement au rafraîchissement, captures simulées étiquetées |
| Rust partagé Unix | `src/frontends/tray/unix.rs` (+723) | `DesktopOverview`, `DesktopAccess`, `DesktopPreferences`, `feedback` borné, projection des accès courants (≤8 lignes, noms ≤256 octets), horodatage d'observation, six commandes de préférences via `Settings::update`, `open_running()` et IPC `open`/`opened` |
| Windows | `src/frontends/tray.rs` (+305), `src/frontends/tray/windows_desktop.rs` (**nouveau**, 361 l.), `src/bin/mcw-tray.rs`, `src/main.rs`, `installer/miccamwatch.wxs` | Fenêtre Win32 retenue à onglets, cases de préférences, rendu DPI/thème, ouverture depuis le premier élément de menu et au double-clic, argument `--open` sur le raccourci du menu Démarrer |
| Linux | `src/frontends/tray/linux_desktop.rs` (**nouveau**, 526 l.) | Fenêtre GTK3 chargée paresseusement par `dlopen` (aucune dépendance Cargo ajoutée), trois onglets, sept langues, erreur explicite si GTK3 ou session graphique manquante, CLI/tray conservés si GTK absent |
| Entrées CLI | `src/frontends/cli.rs` | `mcw tray open` rendu disponible hors macOS (deux attributs `cfg` retirés) |

### C. Travaux inachevés et erreurs de compilation identifiées par inspection

| Problème | Emplacement | Preuve |
| --- | --- | --- |
| `DesktopWindow::is_visible` **n'existe pas** | appelé `src/frontends/tray.rs:1054` | `impl DesktopWindow` ne définit que `create`, `show`, `update`, `destroy` |
| `show_diagnostic_window` et `DETAILS_EDIT` **supprimés** mais encore appelés | `src/frontends/tray.rs:3718` et `:3720` | présents dans `HEAD` (l. 115 et 1902), absents de l'arbre de travail ; le pilote de preuve Windows ne compile donc pas |

Sont également manquants, sans être des erreurs de compilation : le **script de preuve natif Windows**, le **script de preuve natif Linux GTK** et toute **capture réelle** des nouvelles fenêtres.

### D. Tests exécutés et non exécutés

**Exécutés** : audit statique (inspection des symboles, `grep`, lecture des diffs) ; `ast.parse` valide sur `installer/tests/native_macos_menu.py` ; contrôles `git` de périmètre. **Aucun test des interfaces GUI.**

**Jamais exécutés** : `cargo check`, `clippy`, `cargo test` sur les trois cibles ; `swiftc`/`build.rs` du helper macOS ; `native_macos_menu.py` réel (ARM/Intel) ; `app_smoke.swift` sur application construite ; fenêtre GTK3 réelle ; fenêtre Win32 réelle ; vérification du dialogue UAC et des contrôles depuis la nouvelle fenêtre.

**Preuve de non-exécution** : `grep` sur les quatre transcriptions d'agents (`MacGuiRust`, `MacGuiAppKit`, `LinuxFullGui`, `WindowsFullGui`) renvoie **0 invocation `cargo`**, et le seul `bash` du parent entre 05:38 et 05:51 est un `git commit` documentaire.

### E. Linux ARM64 — état réel

- **Développé et commité** (`292e850`, `9ff3471`, `e6ea9a1`, `a4ecf93`, `2998946`) : version `0.21.0`, cible `aarch64-unknown-linux-gnu`, archive dédiée, contrôles ELF64 (machine 62/183) sur les **deux** binaires avant sonde, runner `ubuntu-22.04-arm`, `MISSING_BASELINES` explicite, `release.yml` exige désormais l'asset aarch64.
- **Tests réussis (CI hébergée)** : `39832624207` sur source `2998946` — Windows, Linux x64, **Linux ARM**, macOS Intel et ARM ; `39828453493` — retrait natif géré/Cargo sur quatre Unix, données conservées. Artefacts ARM relus : `uname`/`rustc`/`cc` natifs, glibc 2.35, headers PipeWire/SPA 0.3.48 ARM, 14 refus ELF de l'installateur, root read-only/rollback, six pannes PipeWire sans faux STOP, panneau/menu/autostart sur VM.
- **Non validé** : aucune **machine ARM physique** (Raspberry Pi/SBC), aucune **caméra USB réelle** sur ARM, aucun **bureau ARM réel** (panel/menu), budget 15 Mo non revendiqué, aucune baseline historique ARM64 pour la migration ou les ressources.
- **Non publié** : dernière release `v0.20.0`, aucun tag `v0.21.0`, aucun asset ARM public.

**À retenir** : Linux ARM64 est *compilé et qualifié en machine virtuelle hébergée*, **pas** pris en charge officiellement et **pas** validé matériellement. D'autres architectures (ARMv7, 32 bits, musl) restent hors périmètre.

### F. Modifications transversales (notifications, sons, confidentialité)

| Fichier | Changement | Tests ajoutés |
| --- | --- | --- |
| `src/notify.rs` | `toast_document(..., sound)` : le son du toast Windows obéit à la préférence utilisateur | 1 |
| `src/watcher/unix.rs` | `alert_delivery` : notification et son filtrés **indépendamment** par la pause et les préférences | 1 |
| `src/privacy/macos.rs` | Préflight passif d'inspection caméra : échec passif → consentement administrateur explicite, jamais un état inventé | 4 |

Ces trois correctifs conditionnent les réglages exposés par la nouvelle GUI ; ils n'ont pas non plus été compilés ni exécutés.

### G. Fichiers de cette sauvegarde

12 fichiers suivis modifiés (`+1765/−497`) : `installer/macos/app_smoke.swift`, `installer/miccamwatch.wxs`, `installer/tests/native_macos_menu.py`, `native/macos_desktop.swift`, `src/bin/mcw-tray.rs`, `src/frontends/cli.rs`, `src/frontends/tray.rs`, `src/frontends/tray/unix.rs`, `src/main.rs`, `src/notify.rs`, `src/privacy/macos.rs`, `src/watcher/unix.rs`.
2 fichiers nouveaux : `src/frontends/tray/windows_desktop.rs`, `src/frontends/tray/linux_desktop.rs`.
Plus ce document. **Volontairement exclus** (non supprimés) : `.wrangler/` (cache d'outil), `installer/tests/__pycache__/` (artefacts compilés), `WATCHDOG.yml` (configuration d'agent), `debug_macos/` (utilitaire de diagnostic du 2026-10-05, antérieur au chantier), `docs/next_projects.md` (roadmap du 2026-10-06, remplacée par ce fichier).

### H. Prochaines étapes, dans l'ordre recommandé

1. Ajouter `DesktopWindow::is_visible` et retirer ou réécrire le pilote de preuve qui appelle `show_diagnostic_window`/`DETAILS_EDIT`.
2. Exécuter `cargo check` puis `clippy -D warnings` et `cargo test` par cible (Windows, Linux, macOS) ; corriger les erreurs révélées, non prévisibles par inspection seule.
3. Merger `main` dans la branche (confidentialité + SEO), en résolvant **uniquement** le conflit attendu de `docs/ROADMAP.md` (les deux côtés ajoutent des lignes en fin de journal).
4. Qualifier les surfaces réelles : helper macOS sur ARM et Intel, fenêtre GTK3 sur Linux x64, fenêtre Win32 sur Windows ; créer les deux scripts de preuve manquants.
5. Valider les trois correctifs transversaux (toast/son, gating des alertes, préflight caméra macOS).
6. Linux ARM64 : preuve matérielle Raspberry Pi/SBC (micro observable, caméra USB conforme, bureau réel), puis décision de publication `0.21.0`.
7. Mettre à jour README/roadmap et ne publier qu'après qualification complète.

## Instructions pour le prochain agent IA

1. **Lire cette section et la section précédente avant toute action.** Le dépôt contient du travail non compilé ; ne pas le présenter comme terminé.
2. **Branche de reprise : `work/linux-arm64-021`**, HEAD = commit `wip(0.21.0): preserve GUI implementation and ARM64 handoff`. Ne pas repartir de `main` : `main` ne contient ni GUI ni ARM64.
3. **Ne rien réinitialiser.** Les 12 fichiers modifiés et 2 modules nouveaux sont la seule copie du travail. `.wrangler/`, `WATCHDOG.yml`, `debug_macos/`, `docs/next_projects.md` et `installer/tests/__pycache__/` sont volontairement hors dépôt : ne pas les supprimer, ne pas les committer.
4. **Deux erreurs de compilation connues** sont listées en section C. Les corriger en premier, puis compiler ; toute autre erreur est encore inconnue.
5. **Aucun merge, tag, release ou déploiement** n'a été effectué. `main` (confidentialité, SEO, README) est intact et doit le rester jusqu'à validation.
6. **Ne jamais déclarer** Linux ARM64 « pris en charge », la GUI « terminée », une capture « réelle » ou un test « réussi » sans exécution correspondante. Les seules preuves disponibles sont les runs CI cités en section E.
7. Écrire les commits par lot cohérent, avec preuves ; mettre à jour ce journal à chaque étape franchie.

## 2026-10-23 — Reprise du développement natif 0.21.0

### Base protégée et direction graphique

- Reprise sur `work/linux-arm64-021` depuis la sauvegarde `c4ea35445d68f1e08e846875c68d8c57ecf1b9e9`. Les cinq exclusions locales de la section précédente restent intactes et hors des commits.
- `main` publié (`dc40e95`, confidentialité/SEO/présentation GitHub) intégré **dans la branche de développement** par `c4c7262`. Seul conflit : ajouts documentaires de `docs/ROADMAP.md`, conservés des deux côtés. Aucun changement fonctionnel du site ajouté ; aucun push sur `main`, tag ou release.
- Correction du diagnostic de la CI : le run `39937481249`, source `c4ea354`, échoue sur **`cargo fmt --check` dans les cinq jobs**, avant compilation. Il ne prouve ni une compilation réussie ni des erreurs supplémentaires. Les références Windows orphelines restent un constat d'inspection.
- Direction commune : **Vue d'ensemble / Réglages / Diagnostics**, identité MicCamWatch et logo officiel, typographie native hiérarchisée, grille d'espacement de 8 px et marges de 20–24 px. Deux états courts micro/caméra ; activité observée et protection demandée/effective clairement distinctes ; applications et dernière observation visibles. Les détails SDK et limitations complets restent dans Diagnostics, pas dans le titre principal.
- Exigences : couleurs sémantiques natives, état lisible sans couleur seule, clavier, sept langues, redimensionnement/DPI, thèmes clair/sombre selon les capacités effectivement implémentées et vérifiées. Fermer masque la fenêtre, rouvrir réutilise son état ; aucun contrôle matériel, consentement ou autostart ne doit être déclenché par l'affichage.
- Implémentation répartie sans écriture concurrente : Win32 (`tray.rs`, `windows_desktop.rs`, preuves Windows), GTK3/protocole Rust Unix (`tray/unix.rs`, `linux_desktop.rs`, preuves Linux), AppKit/protocole Swift (helper et preuves macOS). Intégration, notifications/confidentialité, CI et documentation restent sous responsabilité du parent. Aucun build/formatage au milieu des éditions parallèles.
- Précision sur la section F historique : les alertes Unix ont une autorisation générale `notifications_enabled` et une pause communes ; le son exige en plus `sound_enabled` et un début de capture **Active**, pas un accès **Ready**. Les demandes CLI de notification/son sont distinctes ; l'historique est filtré séparément. Ne pas interpréter « indépendamment » comme un son garanti lorsque toutes les notifications sont désactivées.

- Prérequis caméra Linux : `privacy::camera_control_preflight()` réutilise les contrôles en lecture seule de confiance du helper fixe et de `pkexec`, également conservés dans la vraie commande. Un inventaire caméra lisible ne suffit plus à présenter l'action comme disponible lorsque son helper est absent ou dangereux. Ce préflight ne prouve ni agent Polkit, ni consentement administrateur, ni accès effectif à la caméra.
- Thème Windows : activation de l'API WinRT documentée `UI_ViewManagement` pour lire la palette utilisateur ; aucun framework graphique ajouté. Les preuves Windows de fenêtre et de Shell sont séparées pour pouvoir exercer la fenêtre localement sans toucher au tray installé ni au presse-papiers.
- CI : nouvelles preuves natives de fenêtre Windows (`windows-desktop-native.ps1`) et GTK (`native_linux_desktop.py`) prévues dans la matrice existante, avec conservation des preuves ELF/PipeWire/installateur. Les scripts ne constituent pas un runtime de l'application.

**Point de reprise de cette étape :** terminer les trois implémentations existantes et leurs preuves, puis formatage intégré, compilation/tests/lint Windows, qualification CI native Linux x64/ARM64 et macOS Intel/ARM. La fusion documentaire n'est pas une validation GUI ; aucun nouveau résultat de compilation ou de capture n'est encore revendiqué ici. Pas de machine ARM physique ni de serveur distant configuré disponible ; les preuves hébergées ne les remplacent pas.

### Premier contrôle intégré — résultats observés

- Les trois lots natifs sont livrés et le formatage Rust intégré a été exécuté. Les scripts Python macOS/Linux passent `ast.parse` sans créer de bytecode ; le workflow CI se parse correctement en YAML.
- `cargo check --all-targets --locked --features windows-tray` atteint désormais réellement le compilateur Windows. Premier résultat : cinq erreurs de typage `u32`/`COLORREF` sur la palette Win32 et deux imports devenus inutiles ; correctif en cours, aucune réussite de compilation encore annoncée.
- Revue d'intégration : le décodage Swift refuse désormais les JSON UTF-16/32 que Foundation peut transcoder implicitement, sans copie de chaîne supplémentaire. Les dates UTC à fraction nanoseconde restent acceptées sans arrondi au jour suivant ; PID et tailles de projection gardent les bornes du producteur.
- La revue Windows exige encore de séparer le **readback SDK réel** de la demande de protection dans les cartes et de ne pas afficher une caméra inactive si ses collecteurs sont absents. Une tentative de scan échouée doit être nommée comme telle, pas comme une observation réussie. Ces corrections et régressions précèdent les preuves visuelles.

### Qualification Windows — première exécution native

- Après correction des `COLORREF` et des quatre avertissements Clippy (actions LSP, sans désactiver de lint) : `cargo check --all-targets --locked --features windows-tray`, Clippy strict, tests avec et sans `windows-tray` (**186 réussis, 6 ignorés** pour chaque invocation) et build debug réussissent.
- Le pilote Win32 a réellement créé et capturé les quatre onglets à **120 DPI**. Première erreur du pilote : il supposait le test enfant direct du shim `cargo.exe` ; CIM établit `rustup.exe → cargo.exe du toolchain → exécutable de test`. Lancement du Cargo sélectionné via `rustup which cargo`, sans affaiblir la vérification HWND/PID.
- La preuve native suivante atteint les contrôles puis échoue sur **Tab ne quitte pas Diagnostics**. Les captures réelles montrent aussi des boutons trop massifs et une organisation Windows encore trop technique : correction clavier et finition compacte en cours, pas de validation visuelle finale revendiquée. Les données des captures sont explicitement simulées ; aucune commande matérielle ou autostart n'est exécutée.
- Un vrai smoke CLI debug, `mcw --lang fr status --json` dans un APPDATA/LOCALAPPDATA temporaire, révèle un arrêt non déroulant : `slice::from_raw_parts_mut` reçoit le pointeur nul d'une énumération Media Foundation vide (`src/platform/windows.rs`). Le correctif traite l'inventaire vide comme une tranche vide et refuse un pointeur nul avec longueur positive ; la lecture des chaînes SDK suit la même règle. Confirmation runtime après reconstruction encore nécessaire. Ce défaut n'était pas visible dans les tests unitaires ; il justifie de conserver les essais natifs réels.

## 2026-10-23 — Handoff DeepSeek → GPT-6.1 Sol

### État retrouvé avant toute nouvelle correction

- Branche active `work/linux-arm64-021`, HEAD **`c4c7262`** ; aucun commit supplémentaire de DeepSeek trouvé. Le distant reste sur `c4ea354`. Les sept commits d'avance incluent l'historique publié de `main` déjà intégré par Sol, pas sept nouvelles étapes de qualification.
- Quatre worktrees conservés ; le vieux stash `mcw-local-handoff-preserved-before-0.17.2-sync-20261013` n'est ni appliqué ni supprimé. Les cinq exclusions historiques restent hors sauvegarde : `.wrangler/`, `WATCHDOG.yml`, `debug_macos/`, `docs/next_projects.md`, `installer/tests/__pycache__/`.
- Les traces de la conversation établissent les changements DeepSeek dans **`windows_desktop.rs`** et **`windows_desktop_proof.rs`** : prise en compte de `DrawFocusRect`, tentative d'activation du test, parcours Tab explicite, suppression du viewport enfant au profit de contrôles plats avec défilement, conservation du focus dans `layout`, instrumentation des cases à cocher. Le correctif SDK vide et le hint Linux `suppress-sound` existaient déjà à la fin de l'étape Sol ; leur présence n'est pas une nouvelle qualification Unix.
- Le design à cartes, boutons dessinés nativement, en-tête/navigation fixes et trois espaces fonctionnels est conservé. **Le retrait du viewport n'est pas validé visuellement** : le simple masquage des contrôles entièrement sortis de la page ne prouve pas le clipping des contrôles partiellement visibles ni l'accessibilité des éléments cachés au clavier.
- La preuve de focus a été rendue conditionnelle et contient une comparaison de l'ordre Tab avec lui-même ; cela ne remplace pas une preuve de focus réelle. L'explication « limitation de l'environnement » avancée pendant les essais n'est **pas établie** : les logs montrent aussi l'échec lorsque la fenêtre est au premier plan. Sol doit diagnostiquer la cause, sans accepter ce contournement comme validation.

### Résultats confirmés et point d'arrêt

- Sur des états intermédiaires : Clippy strict Windows passe ; tests avec et sans `windows-tray` : **185 tests bibliothèque + 1 CLI réussis, 6 ignorés**. Ces résultats ne qualifient pas automatiquement les dernières modifications de focus, de hiérarchie et d'instrumentation.
- DeepSeek a reconstruit la CLI et exercé `mcw --lang fr status --json` dans un APPDATA/LOCALAPPDATA temporaire : **sortie 0**, collecteurs déclarés sains et zéro accès. Le crash de tranche nulle Media Foundation précédemment reproduit ne s'est pas reproduit après le correctif. Aucune demande de mutation de périphérique.
- Plusieurs essais Win32 échouent sur le focus conservé ou Tab. Dernier essai achevé retrouvé : **`20261023-143918-10328`**, échec de `native_windows_desktop` sur `!Settings::load().unwrap().sound_enabled`. Des captures réelles partielles des onglets anglais existent, à données simulées ; aucune exécution complète sept langues/62 captures n'a réussi.
- Dernière modification : instrumentation `DBG5`/`DBG6` avant/après `BM_CLICK` de la case Son. La commande suivante a été interrompue ; aucun nouveau journal complet correspondant n'est retrouvé. **Ne pas changer l'assertion de préférence pour obtenir artificiellement un succès** ; vérifier l'état natif, la notification reçue et la persistance réelle.
- Dernière CI distante : **`39937481249`**, source `c4ea354`, échec de formatage déjà décrit. Les nouvelles GUI n'ont toujours pas de qualification Linux/macOS distante. La qualification ARM64 de `2998946` reste distincte ; aucun Raspberry Pi/SBC physique.

### Sauvegarde et reprise Sol

Sauvegarder maintenant les **19 fichiers suivis modifiés et 3 nouveaux fichiers utiles** (`native_linux_desktop.py`, `windows-desktop-native.ps1`, `windows_desktop_proof.rs`) dans un commit WIP explicite, y compris l'instrumentation interrompue. Ne pas ajouter les caches, preuves temporaires, secrets ou fichiers personnels. Le site et `PRIVACY.md` sont identiques à `origin/main` au contrôle de reprise ; aucun autre worktree n'est modifié.

Ordre de reprise : (1) diagnostiquer et corriger les événements/focus/cases Win32 avec assertions fortes ; (2) vérifier le clipping et le clavier sans nouvelle refonte arbitraire ; (3) preuve Windows complète et examen des captures ; (4) commits ciblés et push de la seule branche de développement pour les CI natives Linux x64/ARM64 et macOS Intel/ARM ; (5) lire leurs vrais résultats et captures, puis documenter les limites physiques. Aucune release, aucun tag, aucun push sur `main`.

### Reprise Sol — sauvegarde et premières corrections confirmées

- **`7f5fa29`** sauvegarde les 22 fichiers utiles avant correction, instrumentation comprise ; les cinq exclusions restent intactes. **`b98f976`** permet un dispatch CI natif Unix explicite (`native-only`), sans rejouer l'échec Windows connu. Ce mode ne constitue pas une réussite de la CI complète. Aucun push sur `main`, tag, release ou déploiement.
- La CI **`39984579059`** atteint Clippy après un formatage réussi sur les quatre runners Unix. Elle révèle les attributs `cfg` placés sur une affectation et le readback `system_managed` absent de la structure Linux. **`385f3e1`** conserve ce readback réel et transforme le `cfg` en bloc d'instructions.
- La CI suivante **`39986226828`** révèle ensuite deux affectations sur l'inspection Linux immuable et deux lints Rust partagés macOS (`let_and_return`, `collapsible_if`). Corrections intégrées sans désactiver de lint ; les avertissements des dépendances SPA vendoriées ne sont pas confondus avec ces erreurs du projet. Nouvelle compilation native encore à exécuter sur ces corrections.
- Revue GTK : correction d'une course **dans la synchronisation de la preuve native** entre publication/ouverture, rendu et requête d'inspection. Une réponse visible attend désormais la révision demandée et l'ouverture ; les mises à jour cachées restent volontairement différées. Aucune assertion supprimée, aucun contrôle ELF/PipeWire/installateur modifié.
- Revue AppKit : un cadre valide actif/prêt sans identité affichable pouvait prétendre qu'aucun accès n'était observé. Cette assertion est désormais réservée aux deux ressources inactives avec zéro accès ; une explication d'identités indisponibles/omises existe dans les sept langues. Diagnostics reprend le défilement externe existant pour garder une longue action d'inspection et Copier accessibles, en conservant la sélection et les deux niveaux de défilement.
- Les scénarios natifs macOS couvrent ces identités absentes/omises et une action Unicode longue, ainsi que la géométrie et l'accès à Copier à taille minimale. `ast.parse` des preuves Linux/macOS réussit sans écriture de cache ; **cela ne prouve ni une compilation Swift ni un rendu AppKit**.
- Sur Windows, la trace native établit que `BM_SETSTATE(1)` sur l'onglet sélectionné provoquait un `BN_CLICKED` parasite lors du changement de focus. Après son retrait, les assertions fortes de focus/Tab et la préférence Son passent en anglais. Un nouvel arrêt `0xC0000005` a été inspecté en première chance, sans le masquer : activation WinRT `UISettings` à la fenêtre suivante, après initialisation/libération trop courte de l'appartement. Correctifs Win32 et preuve complète en cours ; aucune qualification sept langues annoncée à ce stade.

**Prochain point :** compilation/Clippy et preuve Win32 complète, CI native Linux x64/ARM64 et macOS Intel/ARM sur ces corrections, puis lecture des vrais résultats et captures. La qualification ARM64 hébergée historique reste distincte de toute validation matérielle Raspberry Pi/SBC, toujours absente.

### Fenêtre Win32 corrigée — preuve complète et revue visuelle

- L'appartement WinRT et `UISettings` vivent désormais jusqu'à la fin du thread GUI, au lieu d'être détruits après chaque lecture de palette. L'objet est libéré avant l'appartement ; aucun framework ni dépendance supplémentaire. Le crash d'activation à la fenêtre suivante ne se reproduit pas dans les sept créations/destructions successives de la preuve.
- Le viewport enfant natif est rétabli : les contrôles partiellement défilés restent clippés sous l'en-tête fixe, sans cacher les réglages hors écran au parcours clavier. Les panneaux décoratifs sont placés derrière leurs libellés et cases. Le parcours Tab utilise Win32 sans vecteur alloué à chaque touche. Retrait de l'attachement aux files d'entrée étrangères, des traces `DBG5`/`DBG6`, des pauses de debug et des assertions conditionnelles/tautologiques.
- Sur le PC Windows, Clippy strict, `cargo fmt --all --check`, **186 tests réussis / 6 ignorés**, build debug et `mcw --lang en --help` / `mcw --lang en tray --help` réussissent. Cette dernière commande expose bien `tray open`. Les tests sans feature précédemment réussis ne sont pas relancés inutilement sur du code hors de leur configuration.
- Preuve réelle **`20261023-185329-28712`** : **62 captures, 7 langues, 120 DPI**, préférences désirées persistées/idempotentes, portes de capacité, profil explicite, focus/Tab/sélection/défilement, masquage/réouverture et palette utilisateur. Une régression supplémentaire vérifie qu'un réglage hors écran reçoit le focus et devient entièrement visible à taille minimale ; la preuve **`20261023-190341-21800`** passe également, avec capture française du réglage révélé.
- Captures examinées : Vue d'ensemble anglaise claire/sombre, Réglages russes, tailles minimales allemandes/japonaises, réglages français/chinois défilés et Diagnostics espagnols. Cartes et libellés sont visibles, navigation fixe et commandes accessibles. Les palettes claire/sombre du client sont simulées par la preuve, **pas un changement du thème global Windows**. L'apparence réelle utilise la palette utilisateur. Lecture d'écran, autres DPI/moniteurs et bascule globale de thème restent à qualifier manuellement.
- Toutes ces observations emploient des données explicitement simulées et des préférences temporaires ; **zéro commande matérielle et zéro commande autostart exécutées**. Le tray installé et le presse-papiers utilisateur restent intacts. Un blocage de lien `LNK1104` après attachement du débogueur a disparu après fermeture du processus de diagnostic ; il n'a pas été traité par une modification de l'application.
- CI **`39989884264`**, source **`409a3c6`** : formatage, Clippy strict, tests Rust, builds debug/release passent sur **Linux x64/ARM64 et macOS Intel/ARM**. ELF Linux, en-têtes SPA, payload utilisateur/frontière root, profils Linux, app/DMG macOS, inspection/cancellation et bootstrap/mise à jour portable macOS passent. Les preuves des nouvelles fenêtres **GTK et AppKit échouent encore** ; leurs traces et captures doivent être analysées avant qualification GUI. Windows était volontairement exclu de ce dispatch Unix, pas déclaré réussi par ce run.

### Propriété COM réelle et seconde passe de qualification native

- **`cfb3025`** sauvegarde et pousse la fenêtre Win32 et sa preuve précédente. Le contrôle du vrai appelant (`PlatformMonitor::_com`, `src/platform/windows.rs`) établit toutefois un appartement **MTA** existant. La nouvelle régression libère la référence de cet appelant après création de la fenêtre : **avant correction**, `CoGetApartmentType` échoue avec **`0x800401F0`**, puis la fermeture provoque **`0xC0000005`** (`20261023-194831-19528`). Ce cas n'était pas couvert par le thread GUI autonome initial.
- Le runtime GUI prend désormais sa propre référence WinRT dans l'appartement existant : après `RPC_E_CHANGED_MODE` d'une demande STA, il initialise le MTA ; seules les initialisations réussies sont libérées. `UISettings` n'est pas activé si l'initialisation échoue. Aucun droit administrateur n'est requis ou demandé pour cette lecture de palette.
- **Après correction**, Clippy strict, les **186 tests / 6 ignorés**, le build debug et la preuve **`20261023-195911-25728`** passent. La preuve vérifie l'appartement MTA explicite après libération de l'appelant, puis les sept langues et 62 captures avec les assertions fortes précédentes. Capture de la vue d'ensemble effectivement examinée ; aucun contrôle matériel/autostart.
- GTK, échec `39989884264` : l'assertion agrégée de refresh échoue sur les deux architectures. Le traitement GLib borné pouvait consommer le viewport sauvegardé avant la fin des validations incrémentales du texte ; il est maintenant conservé jusqu'à drainage du contexte, y compris entre publications successives. La prochaine preuve conserve toutes les assertions et écrit `refresh-before.json` / `refresh-after.json` avec les différences par champ : le lien avec le champ exact de cet échec reste **à confirmer nativement**. Les libellés d'en-tête décoratifs ne capturent plus le focus initial sous forme d'une sélection bleue ; les applications et Diagnostics restent sélectionnables.
- AppKit, même run : les captures Intel/ARM montrent l'action longue entière, mais la fenêtre dépasse la taille demandée au lieu de faire défiler son document. Retrait du couplage fort document/viewport ; préférence faible indépendante pour la hauteur du texte. La preuve vérifie la taille réellement demandée et ses limites déclarées par AppKit, sans agrandir artificiellement le cas minimal, et conserve overflow/accès à Copier/sélection/défilement.
- Les PNG AppKit transparents étaient trompeurs dans le visualiseur : ils sont désormais composités sur le **vrai fond de fenêtre**, dans son apparence native, avec dimensions/échelle/provenance par image. Ce sont des captures de vues AppKit réellement montées, pas des captures WindowServer ni des mocks de menu ; les PNG de texte du menu restent explicitement des fixtures. Aucun nouveau sous-processus de capture, demande de permission ou changement de palette de l'application.
- Préférences transversales : `EventDispatcher` relit les préférences, applique la désactivation de l'historique et les portes notifications/pause/son ; le frontend Unix recharge aussi son état de préférences. Les tests Rust natifs ont passé les cas Active/Ready/Start/Update/Stop et les portes de pause/désactivation. Le hint Linux `suppress-sound` demande au daemon de ne pas ajouter son son par défaut ; il ne garantit pas le comportement de chaque bureau ni l'audibilité d'un son.

**Point de reprise :** pousser ces correctifs ciblés sur `work/linux-arm64-021`, exécuter la CI complète sur Windows/Linux x64/Linux ARM64/macOS Intel/macOS ARM, lire les nouveaux états et PNG avant de clore les GUI. Les builds/ELF/SPA/PipeWire natifs ARM64 réussis sur runner Ubuntu 22.04 (glibc 2.35, `e_machine=183`) ne remplacent toujours pas un essai Raspberry Pi/SBC ou caméra USB physique. Ne pas relancer les preuves locales Windows déjà réussies sans nouveau changement affectant leur chemin.

**CI complète déclenchée :** [`40001797600`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40001797600), source **`28a7562`**, `native-only=false` : Windows et les quatre cibles Unix, sans release ni déploiement. Attendre ses résultats et récupérer ses preuves avant une nouvelle correction ; ne pas rejouer par défaut les scénarios locaux déjà passés. Le commit `28a7562` est poussé sur la seule branche `origin/work/linux-arm64-021`.

### Résultats `40001797600` et derniers écarts circonscrits

- **Linux x64 et ARM64 : jobs complets réussis**, nouvelles fenêtres GTK, préférences, cycle de vie, headless, profils, statut/menu, builds, ELF/SPA, protections et payload utilisateur compris. Les états natifs sauvegardés montrent la même fenêtre GTK, le même onglet, focus, sélection et défilement (**90 → 90**) pendant le refresh. La régression initiale n'est plus observée ; aucune assertion n'a été assouplie. Les dernières modifications ne touchent plus au chemin Linux.
- **Windows :** CLI avec/sans tray, Clippy, tests, audit des dépendances, build release, owners zéro périphérique/IPC, budgets de ressources, vraie mise à jour avec tray actif, profils, nouvelle fenêtre et smoke MSI réussissent. Seul le contrôle Shell/tooltip échoue ; ne pas présenter ce job global comme réussi.
- Le JSON Shell établit un rectangle UIA devenu **offscreen**, le curseur sur **WindowsTerminal** et un rectangle de l'icône désormais différent. Le pilote quittait l'overflow **après** avoir acquis ce rectangle. Il quitte désormais le survol précédent **avant** la recherche/réouverture de l'icône ; la vérification du propriétaire et le refus de cliquer hors de sa zone restent intacts. Confirmation encore requise sur desktop CI isolé ; aucun déplacement de souris du PC utilisateur pour cet essai.
- **AppKit Intel :** `producer-json-escaped` fournit un feedback valide de 246 lignes ; l'en-tête hors scroller atteint 3865 points malgré `maximumNumberOfLines=2`. Ajout d'une borne de hauteur dérivée de la police, sans tronquer l'évidence complète dans Diagnostics/tooltip/accessibilité ; régression de refresh multiligne dans les langues existantes.
- **AppKit ARM :** la demande roomy de 700 points dépassait la zone visible du display 1024×768 (menu/Dock), d'où un clamp natif à 681. Seule cette fixture roomy s'adapte maintenant à `NSScreen.visibleFrame`. Taille utilisateur et minimum natif inchangés ; dimensions demandées et présence complète dans la zone visible restent exigées. Les vrais cas longs d'inspection et de réglages ont passé avant le cas de feedback Intel.
- Les PNG AppKit opaques permettent maintenant une revue lisible : Vue d'ensemble anglaise sombre effectivement examinée, états Active/Unknown et contrôle effectif distincts, navigation et typographie natives. Les nouveaux correctifs de feedback/taille nécessitent encore leurs captures et résultats.
- Le dispatch CI dispose de **`native-platform=all|linux|macos`**, avec `native-only` séparé pour Windows. Parse YAML/JSON et parse AST PowerShell réussis ; la matrice complète par défaut et la porte Windows sont identiques à `28a7562`. La prochaine qualification cible **Windows + macOS**, sans rejouer Linux dont les sources sont inchangées et les deux jobs réussis.

**Qualification ciblée en cours :** [`40009672564`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40009672564), source **`3c4ea75`**, `native-platform=macos`, `native-only=false` : Windows + macOS Intel/ARM. Les deux jobs Linux restent qualifiés par `40001797600` sans changement de leurs sources. Inventaire des preuves GTK : **21 combinaisons langue/thème + headless par architecture** ; captures représentatives des sept langues, thèmes clair/sombre/contraste élevé et tailles minimales effectivement examinées. Le titre n'est plus sélectionné au premier affichage ; détails, réglages et navigation restent lisibles dans les captures contrôlées.

### Qualification native terminée et prérequis de la mesure Windows

- **`40009672564`, source `3c4ea75` : les deux jobs macOS Intel/ARM réussissent intégralement.** Les preuves AppKit passent les sept langues, quatre apparences natives, tailles utilisateur/minimale/roomy, préférences, clavier, refresh, feedback multiligne et inspection longue. Chaque architecture produit **1 008 captures de surfaces / 1 098 images natives avec provenance** ; ce comptage n'est pas une validation matérielle.
- Captures effectivement examinées : sept langues, Vue d'ensemble/Réglages/Diagnostics, Aqua/DarkAqua/contraste élevé et minimum ; cas `producer-json-escaped` minimal, inspection longue défilée jusqu'à Copier et fenêtre roomy ARM contenue dans le display. Le feedback reste borné, l'en-tête/navigation visibles, les diagnostics complets accessibles par défilement. PNG opaques de vues AppKit montées et compositées sur le fond natif, **pas captures WindowServer**.
- **Windows : nouvelle fenêtre et Shell/tooltip réussissent**, ainsi que profils, transaction avec tray actif et smoke MSI. Le job global échoue toutefois dans `native_resources.py` : les références **0.16.1** et candidate **0.21.0** quittent `watch` pendant le warmup. Les lignes de statut et les trames TUI établissent `privacy_store` et `module_scanner` indisponibles : la clé **HKCU `ConsentStore\webcam` manque**. `webcam_consent_store()` et `ensure_collectors_available()` refusent volontairement ce cas ; ce refus ne doit pas devenir un faux « aucun accès ».
- Le pilote de mesure prépare désormais, **uniquement après les gardes desktop GitHub-hosted et absence de tray utilisateur**, un vrai magasin webcam vide s'il manque. Aucune valeur d'autorisation, entrée de capture, préférence ou politique système n'est écrite ; les ACL existantes ne sont pas modifiées. Un magasin existant est seulement ouvert en lecture. Ce prérequis et son initialisation éventuelle sont enregistrés dans `resources.json` ; la clé de fixture reste dans la VM jetable jusqu'à sa destruction. Le code produit et ses assertions de collecteurs restent inchangés.
- Essai CLI local séparé, APPDATA temporaire et notifications/son/historique/contrôles sur verrouillage désactivés : `watch --json --no-kill --interval 200` survit aux quatre contextes console testés (héritée, détachée, sans fenêtre, console cachée propre). Seuls les processus de cet essai sont arrêtés par leur handle. Cela écarte une dépendance évidente à la console locale, **pas une reproduction sur le runner sans clé** ; aucune clé du PC utilisateur n'est modifiée.
- Le dispatch ajoute **`native-platform=windows`** pour rejouer le seul job Windows, avec tous ses garde-fous. Les matrices et comportements par défaut restent inchangés ; Linux qualifié par `40001797600` et macOS qualifié par `40009672564` ne sont pas rejoués sur des sources inchangées.

**Prochain contrôle :** validation syntaxique et périmètre de ces seuls changements de preuve/CI/documentation, commit ciblé sur la branche de développement, puis qualification Windows seule et lecture de `windows_consent_store_fixture`, des résultats fonctionnels et des mesures réelles. Les budgets ne sont ni relevés ni présentés comme satisfaits à partir d'une exécution interrompue. Aucune release ni validation ARM physique.

### Transmission Sol — périmètre sauvegardé et limites de qualification

- **`1fe846e`**, poussé uniquement sur `origin/work/linux-arm64-021`, contient le prérequis de mesure Windows et le sélecteur Windows seul. Parse Python AST et YAML réussis ; la matrice Unix par défaut et les étapes de garde Windows sont conservées. Run ciblé [`40014935046`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40014935046), `native-platform=windows`, `native-only=false` : **terminé avec succès**, résultat et artefacts lus ci-dessous.
- Chaîne de sauvegarde/corrections : **`7f5fa29`** (travail interrompu intégral), **`b98f976`** (dispatch Unix), **`385f3e1`** (compilation Unix), **`409a3c6`** (états natifs/overflow), **`cfb3025`** (événements/viewport Win32), **`28a7562`** (propriété WinRT MTA/scroll GTK/géométrie AppKit), **`3c4ea75`** (feedback AppKit/hover Shell), **`1fe846e`** (prérequis de preuve Windows). Les changements DeepSeek ont d'abord été sauvegardés, puis corrigés lorsqu'une preuve les contredisait, sans supprimer la conception native.
- Fichiers GUI : `native/macos_desktop.swift`, `src/frontends/tray.rs`, `src/frontends/tray/{unix,linux_desktop,windows_desktop,windows_desktop_proof}.rs`, `src/bin/mcw-tray.rs`, `Cargo.toml`. Corrections transversales conservées : `src/platform/windows.rs`, `src/notify.rs`, `src/notify/unix.rs`, `src/privacy/{linux,macos}.rs`, `src/watcher/unix.rs`.
- Preuves/documentation : `.github/workflows/ci.yml`, `installer/macos/app_smoke.swift`, `installer/tests/{native_linux_desktop,native_linux_tray_width,native_macos_menu,native_resources}.py`, `installer/tests/{windows-desktop-native,windows-tray-native}.ps1`, `README.md`, `docs/ROADMAP.md`. `app_smoke.swift` est un pilote de qualification, **pas une modification de l'installateur**. Aucun installateur ou paquet public n'a été modifié par cette reprise.
- Préférences : états désirés persistés et idempotents, réouverture conservée, profil explicite respecté, refus de capacité non déguisé en succès. Les notifications exigent activation et absence de pause ; le son exige en plus son activation et un début **Active**, pas Ready/Update/Stop. Les preuves valident les portes et les API natives ; **ni audibilité physique, ni livraison garantie par tous les daemons de bureau** ne sont revendiquées.
- Les sources du site et `PRIVACY.md` sont toujours identiques à `origin/main`, ancêtre de la branche de développement. Le distant `main` reste **`dc40e95bc9c1c6a14386eb5e773f01b62b47aa71`** au contrôle. Les cinq exclusions locales, le stash et les autres worktrees ne font l'objet d'aucune modification ou publication.

#### Ressources : résultats observés, pas une promesse universelle

Les mesures suivantes concernent **le tray au repos, fenêtre principale non ouverte**, arbre de processus produit inclus. MB décimaux ; mémoire des serveurs de bureau hors périmètre. RSS partagée susceptible d'être comptée plusieurs fois ; ce tableau n'est ni une mesure de PSS ni un budget de fenêtre GUI ouverte.

| Cible et preuve | Pic RSS agrégé observé | Médiane RSS soutenue | Limite de la preuve |
| --- | ---: | ---: | --- |
| Windows x64 — `40014935046` | 27,40 MB | 16,78 MB | Objectif 15 MB non atteint ; 4 intervalles dépassent 50 ms |
| Linux x64 — `40001797600` | 13,03 MB | 12,89 MB | Objectif 15 MB observé sur ce scénario, pas preuve de tous les pics possibles |
| Linux ARM64 — `40001797600` | 11,45 MB | 11,45 MB | Pas de référence ARM64 0.16.1 ; aucune comparaison x64 substituée |
| macOS Intel — `40009672564` | 37,70 MB | 30,22 MB | Objectif 15 MB non atteint ; 125 intervalles de mesure dépassent 50 ms |
| macOS ARM — `40009672564` | 55,07 MB | 42,52 MB | Objectif 15 MB non atteint ; coûts du helper AppKit inclus |

L'objectif historique 15 MB reste **souple et non atteint sur Windows et macOS** ; le repère 30 MB n'est pas un seuil de réussite fonctionnelle. Aucun budget, cadence ou coût natif n'est masqué pour obtenir un job vert. La mesure Windows interrompue `40009672564` reste non qualifiée ; celle de `40014935046` se termine avec comparaison référence/candidate qualifiée.

#### Instructions pour le prochain agent IA — état courant

1. Lire les dernières lignes de cette transmission et les résultats du run ciblé avant toute action. Les mentions historiques « non compilé » plus haut décrivent le point d'arrêt ancien, **pas les compilations natives réussies depuis**. Ne pas revenir à `c4ea354` ou reprendre les fenêtres de zéro.
2. Travailler sur `work/linux-arm64-021`, vérifier HEAD, différences, worktrees et cinq exclusions avant une nouvelle édition. Aucun reset/nettoyage/stash destructif ni modification des autres worktrees. Les preuves volumineuses sont des artefacts CI/temporaire, pas des fichiers à ajouter à Git.
3. Ne rejouer que les chemins affectés : Linux x64/ARM64 qualifiés par `40001797600`, AppKit Intel/ARM par `40009672564`, Windows complet par `40014935046`. Une nouvelle modification de source invalide les résultats des chemins qu'elle affecte ; une modification documentaire seule ne nécessite pas une nouvelle compilation.
4. **ARM64 matériel reste ouvert** : vrai Raspberry Pi/SBC, distribution/glibc/GTK/PipeWire/Polkit, microphone/caméra USB, droits, branchements à chaud et récupération d'owners à vérifier. ELF64 little-endian `e_machine=183`, glibc 2.35 et session PipeWire hébergée ne prouvent pas un support Raspberry Pi OS, ni des glibc plus anciennes.
5. QA manuelle restante : Windows 10, autres DPI/moniteurs et thème global réel ; Linux Wayland/autres bureaux et lecteurs d'écran ; macOS sur poste utilisateur avec autorisations réelles et ressources physiques. Les captures des fenêtres natives utilisent des observations simulées explicites ; les API, préférences et contrôles de géométrie sont réellement exécutés, pas une preuve de capture physique.
6. Préserver confidentialité/SEO de `main`, les droits et les avertissements de surveillance incomplète. Aucune release 0.21.0, aucun tag, push `main`, déploiement ou annonce de support matériel ARM sans autorisation et preuves supplémentaires.

### Résultat final de qualification native — 2026-10-23

- **Windows complet réussi** : [`40014935046`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40014935046), source **`1fe846e954fc87560cbd3a08d605f5dfa67a6e89`**. `windows=success`, `portable=skipped` comme demandé. Formatage, tests CLI avec/sans tray, Clippy strict, audit des dépendances, release, owners/IPC, terminal, ressources, restart avec tray vivant, transaction ancienne version → candidate, profils, fenêtre Win32, Shell/tooltip et smoke MSI passent. Aucun changement de l'installateur.
- `resources.json` : **14 scénarios fonctionnels réussis**, référence 0.16.1 et candidate 0.21.0 complètes, `comparison_qualified=true`. `watch` survit à sa période mesurée puis est arrêté par son handle retenu ; son code de terminaison forcée Windows n'est pas confondu avec une sortie spontanée au démarrage. Les limites de cadence/objectif mémoire restent visibles et non satisfaites là où mesurées.
- **Limite explicitement vérifiée :** ce runner possédait déjà la clé webcam (`initialized_after_missing=false`). Ce run valide donc le chemin de préservation du magasin existant, **pas la création conditionnelle à lui seul**. Un script jetable séparé a exécuté les instructions réelles de la fixture, extraites de leur AST, dans un **HKCU redirigé uniquement dans son processus enfant** vers une clé privée neuve : création du magasin absent et vide réussie, puis valeur d'autorisation synthétique et sentinelle binaire préexistantes conservées lors du second passage. Le sandbox possédé a été supprimé ; aucune clé caméra réelle, commande produit, autorisation, action matérielle ou marqueur GitHub-hosted falsifié. Un premier essai du script s'était arrêté sur son extraction AST avant toute création de clé ; seule cette extraction temporaire a été corrigée.
- Preuve Win32 finale **`20261023-201740-8744`** : **62 captures / 7 langues**, focus clavier vérifié, préférences/capacités/profil explicite, conservation sélection/scroll et fermeture/réouverture. `device_actions_executed=0`, `autostart_actions_executed=0`, `hardware_validation=false`. Preuve Shell **`20261023-201804-5036`** : huit popups réellement observés, pointeur/clic limités à l'icône possédée, aucune case échouée.
- Les cinq cibles ont désormais leurs **compilations et scénarios natifs réussis** sur les sources concernées : Linux x64/ARM64 (`28a7562`, `40001797600`), macOS Intel/ARM (`3c4ea75`, `40009672564`), Windows (`1fe846e`, `40014935046`). Les écarts historiques ci-dessus ne sont pas des échecs encore persistants de ces scénarios ; les validations physiques/manuelles et les objectifs mémoire restent des limites distinctes.
- **`7006ea2`** a déjà sauvegardé le périmètre, les preuves et les instructions de reprise ; cette dernière réception est documentaire seulement. Branche à poursuivre : **`work/linux-arm64-021`**, aucun retour au vieux WIP. Prochain travail : QA sur postes réels et matériel ARM décrite ci-dessus, puis revue de qualification avant toute proposition de publication. Pas de nouvelle refonte GUI, pas de reprise inutile des tests déjà passés ; aucun push `main`, tag, release ou déploiement effectué.

## 2026-10-24 — Une application MicCamWatch : lancement, identité et propriété

### Décisions et état retrouvé

- Base protégée : `a29dcc5` sur `work/linux-arm64-021`, synchronisée ; cinq exclusions, stash et autres worktrees conservés. Aucune réorganisation du README, modification du site/confidentialité, release, tag ou push `main` dans ce lot.
- **Windows conserve deux exécutables, pas deux applications utilisateur.** `mcw.exe` est la CLI et peut héberger un frontend explicitement lancé depuis le terminal ; `mcw-tray.exe`, sous-système Windows, est l'entrée sans console. Le MSI est le paquet Windows Installer, pas un troisième programme résident. Les services de contrôle peuvent utiliser des processus supplémentaires sans créer de fenêtres principales supplémentaires.
- Le MSI définit déjà **un seul produit**, un seul raccourci « MicCamWatch » vers le compagnon avec `--open`, une icône officielle et l'identité `MicCamWatch.MicCamWatch`. La séparation évite une console lors du lancement graphique et conserve les commandes avancées ; aucune fusion physique des exécutables n'est nécessaire.
- Audit local **en lecture seule** : un produit MSI 0.20.0 et une installation Cargo coexistent ; le raccourci vise `%LOCALAPPDATA%\Programs\MicCamWatch`, alors que le tray actif et la première résolution de `mcw` viennent de `%USERPROFILE%\.cargo\bin`. Ce sont deux racines d'installation réelles, distinctes de la paire CLI/GUI normale. Aucun processus installé, droit, PATH, autostart ou protection de ce PC n'a été modifié. Une éventuelle consolidation vers le MSI pour l'usage bureau doit être explicitement autorisée, avec restauration et sortie normales avant retrait d'une copie ; ne pas supprimer Cargo ni voler son owner.

### Corrections et contrats d'intégration

- **Win32** : réutilisation authentifiée de la paire installée et de son namespace APPDATA/LOCALAPPDATA, acquittement de l'ouverture réelle, permission de premier plan seulement via l'API documentée. La contention du mutex au démarrage attend un owner disponible ou acquiert normalement une lease libérée ; aucune seconde série de workers. Une ouverture explicite révèle la même fenêtre ; un lancement de fond n'ouvre pas de fenêtre. Un `--config` explicite ne remplace pas silencieusement la politique d'un owner existant.
- La fenêtre principale Windows reçoit `WS_EX_APPWINDOW` en plus du contrôle clavier : elle ne doit plus être traitée comme la simple fenêtre possédée d'un tray caché. L'AUMID explicite du processus, le raccourci et les notifications partagent la même constante statique ; affichage officiel « MicCamWatch ». Le compagnon rend désormais un code d'échec après son dialogue d'erreur, au lieu de toujours sortir avec succès.
- **Propriété MSI** : `mcw update` refuse le remplacement binaire d'une installation enregistrée avant verrou, réseau, staging ou arrêt d'un frontend. Il indique le MSI officiel et le produit concerné ; **ce n'est pas une mise à jour MSI automatisée**. Windows Installer conserve la version enregistrée, la réparation, les fichiers, le PATH et les raccourcis. La mise à jour transactionnelle portable et la délégation de désinstallation existantes restent distinctes. Restaurer explicitement les protections et quitter normalement avant une opération MSI ; les preuves de retrait ne démontrent pas une désinstallation forcée avec protections actives.
- **Unix commun** : le lock est acquis avant le bind du socket ; une activation simultanée à l'autostart pouvait perdre sa demande d'ouverture. Contention typée, attente bornée et contrôle authentifié préservent maintenant l'ouverture ; un owner disparu libère normalement sa lease. Un doublon de fond attend la disponibilité native sans révéler la fenêtre. Erreurs de droits, images ou namespaces étrangers restent des refus, pas des invitations à forcer un autre processus.
- **Linux, installateur utilisateur 0.21+** : une entrée XDG `com.roman-cuisset.miccamwatch.desktop` ouvre `mcw tray open`, avec l'icône officielle 96×96 et sans activer l'autostart. GLib/GDK déclarent l'identité applicative et `StartupWMClass=MicCamWatch`. L'entrée de session existante demeure séparée et en fond.
- Les assets Linux `desktop`/`icon` font partie du journal transactionnel existant : chemin d'origine fixé dans les reçus, hashes, refus de liens/hardlinks/ancêtres dangereux/fichiers étrangers ou modifiés, rollback et retrait uniquement des assets possédés. Les anciens reçus sans ces quatre champs sont reconnus comme anciens, jamais comme une autorisation d'adopter un fichier tiers.
- **Premier upgrade Linux depuis 0.20** : son ancien updater embarque un installateur sans lanceur. L'appel post-installation existant `autostart refresh` déclenche désormais une migration locale et hors réseau, via le même producteur d'assets et un mode interne `--refresh-desktop`. Ni binaire mappé ni payload caméra ni intention d'autostart ne sont remplacés. Les installations non gérées sont des no-op ; la recette Arch/AUR historiquement épinglée à 0.20 n'est pas réécrite pour prétendre supporter une commande GUI absente.
- **macOS** : le launcher Finder reste éphémère ; un frontend Rust et son unique helper AppKit/accessory possèdent le status item et la consultation retenue. Les identifiants techniques du bundle extérieur et du helper ne signifient pas deux applications résidentes. Ajout de preuves LaunchServices, démarrage caché et activation Finder pendant le créneau lock-avant-socket ; aucune nouvelle architecture ni refonte AppKit.

### Design, preuves et point de contrôle

- Revue des captures AppKit Intel/ARM antérieures : cinq images de Vue d'ensemble/Réglages/Diagnostics, clair/sombre/minimum, sans nouveau défaut démontré justifiant un restylage. Les composites natifs et les données simulées restent explicitement identifiés. L'identité de fenêtre/taskbar et l'accès à la même fenêtre font partie de l'ergonomie, pas seulement de la compilation.
- Nouveaux scénarios de preuve, **encore à exécuter en CI** : vraie installation MSI, vrai `.lnk`, lancements CLI/GUI concurrents, une seule lease/PID/HWND retenue, masquage/réouverture, absence de console observée, refus de self-update MSI sans changement ni requête réseau ; lanceur Gio Linux, identité X11 et migration du vieux producteur ; activation Finder avec barrière de test pré-bind. Les barrières restent dans les pilotes de preuve, pas dans le produit.
- Vérifications déjà exécutées : parse AST des trois scripts Python modifiés, YAML des deux workflows, AST PowerShell et compilation inerte de ses deux groupes C#, `sh -n` de l'installateur ; icône embarquée identique à l'asset canonique. Premier contrôle Windows arrêté sur un nom d'API obsolète (`Error::from_win32`) ; corrigé vers l'API du SDK utilisé (`from_thread`), sans désactiver de lint.
- Après correction : **Clippy strict Windows, 186 tests réussis / 6 ignorés et build debug réussis**. La preuve de fenêtre inerte **`20261024-053642-5988`** réussit : **62 captures, sept langues, 120 DPI**, focus/sélection/scroll/préférences et fermeture/réouverture ; zéro action périphérique/autostart, tray installé intact. Vue d'ensemble claire, Réglages sombres et français effectivement examinés. Palettes client simulées, pas bascule du thème global. Le garde du nouveau pilote MSI refuse réellement le PC non hébergé avant toute création de fixture. Les intégrations installées Linux/macOS/Windows exigent encore leur exécution native sur les runners.
- Prochain contrôle : sauvegarder le lot vérifié localement, puis CI native sur les cinq cibles affectées et examen des vrais résultats/images. Les jobs du 23 octobre ne qualifient pas automatiquement les nouvelles intégrations. Raspberry Pi/USB physiques, Wayland réel, lecteurs d'écran, Windows 10/DPI multiples, login/reboot et regroupement Dock/taskbar restent des frontières manuelles distinctes. Aucun support ARM physique ni objectif mémoire nouvellement réussi n'est annoncé.

**Sauvegarde et qualification déclenchée :** `29e0f32` est poussé sur la seule branche `origin/work/linux-arm64-021` (22 fichiers utiles, cinq exclusions locales intactes). Run [`40059639912`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40059639912), source `29e0f32`, `native-platform=all`, `native-only=false` : Windows, Linux x64/ARM64 et macOS Intel/ARM. Attendre ses résultats et ses artefacts avant de revendiquer la nouvelle intégration installée ; la preuve locale Windows précédente n'est pas une preuve MSI ou ARM physique.

### Résultats de la première qualification et corrections ciblées

- Run **`40059639912`**, source **`29e0f32`** : formatage, Clippy strict, tests Rust et compilations debug/release passent sur **Windows, Linux x64/ARM64 et macOS Intel/ARM**. Les contrôles Linux ELF/SPA et de frontière root passent également. Ces succès ne masquent pas les trois échecs d'intégration ci-dessous.
- **MSI Windows réellement réussi**, même si le job Windows global échoue plus tôt sur le tooltip : un produit et un raccourci installés, vraie activation du `.lnk`, lancements GUI/CLI séquentiels et concurrents, même owner et même HWND après fermeture/réouverture, démarrages de fond cachés, aucune console observée. Le refus de `mcw update` conserve le frontend, les fichiers et l'enregistrement MSI : code 2, zéro connexion proxy et zéro staging. Les icônes des deux exécutables, du raccourci et des applications installées correspondent à l'icône officielle aux neuf tailles testées ; désinstallation normale de la fixture réussie, autres entrées PATH préservées. `VERSIONINFO`/FileVersion des exécutables reste une limite constatée de cette preuve, distincte de la version MSI enregistrée. La capture de la vraie fenêtre installée a été examinée ; le regroupement taskbar reste à vérifier manuellement.
- **macOS ARM : job entier et preuve copied-app réussis** sur cette source, y compris ouverture Finder à froid pendant le créneau pré-bind, owner unique, démarrage caché et fenêtre retenue. **Intel : échec `cold-native-window`**, helper natif/accessory et owner conservés, mais fenêtre non accessible avant l'échéance AX. Les anciens artefacts ne prouvent pas quelle branche d'activation a échoué : ne pas attribuer définitivement cet échec à une cause sans la nouvelle preuve.
- **Linux x64 et ARM : Gio rejette réellement l'entrée produite**, malgré une syntaxe Exec correctement échappée. GLib vérifie `argv[0]` avant l'expansion des codes `%` : un chemin contenant un pourcentage littéral devient inexistant à cette étape. Les producteurs d'application et de session utilisent désormais universellement `/usr/bin/env -- <cible absolue échappée> tray open|run`, sans shell, et `TryExec` vérifie séparément la cible réelle. Les reçus pré-0.21 reconstruisent uniquement leur producteur historique exact ; le refresh migre vers le nouveau format sans accepter d'anciens octets sous un reçu courant. Le scénario Gio conserve les vrais fichiers et vérifie aussi le refus lorsque son exécutable privé est momentanément absent. Référence technique : [GLib 2.72.4, `gdesktopappinfo.c`](https://raw.githubusercontent.com/GNOME/glib/2.72.4/gio/gdesktopappinfo.c), validation TryExec et argv[0].
- **Windows Shell : défaut de pilote démontré**, pas preuve d'un second frontend. L'artefact `recovered-active-shell-observed.json` montre un ancien rectangle UIA devenu hors écran alors que le rectangle natif possédé est maintenant dans la taskbar. Le pilote suit désormais uniquement un centre natif frais contenu dans la cible UIA authentifiée ; même échéance et même obligation d'observer le vrai popup, aucun clic sur une position périmée. La capture MSI révèle aussi des titres dans une famille différente du corps sur ce runner : les titres sont maintenant dérivés de la même métrique native de menu, au lieu de la police de boîte de message indépendamment configurable.
- **macOS : famine d'ouverture démontrée par inspection**, indépendamment de la branche exacte de l'échec Intel. L'ancien ACK ne faisait que poser un drapeau lu avant une collecte susceptible d'attendre jusqu'à 15 s pour les profils caméra, en plus des autres observations. Le serveur existant envoie maintenant la commande show par le transport stdin AppKit sérialisé dès qu'il est prêt ; les demandes antérieures restent en attente. Fermeture/déconnexion refuse les nouvelles demandes, y compris avec readiness périmée. Pas de nouveau worker, de reset du modèle, d'appel matériel ni de modification du protocole. Le pilote Finder capture maintenant les stderr/stdout du vrai enfant LaunchServices et les erreurs AX/WindowServer en cas d'échec.
- Après ces corrections : **Clippy strict Windows réussi**, AST Python et syntaxe POSIX de l'installateur réussis ; AST du pilote Shell réussi sans action sur le tray installé. Les nouvelles assertions portent sur migration/refus de propriété et état de transport, pas sur des copies de chaînes ou un faux écho AppKit. Les scénarios natifs corrigés restent à exécuter avant de clôturer les trois échecs. Prochain point : sauvegarde ciblée, CI des cinq cibles affectées, lecture des reçus réels et des captures ; aucun résultat matériel ARM, login/reboot, lecteur d'écran ou Windows 10 n'est déduit des runners.
- Preuve Win32 inerte après harmonisation typographique : **`20261024-063107-23820` réussie**, 62 captures et sept langues ; focus/sélection/scroll, préférences et fenêtre retenue vérifiés, zéro action périphérique/autostart. Elle ne contacte pas l'installation Cargo/MSI réelle et ne remplace pas la nouvelle qualification Shell/MSI hébergée.

**Correctifs sauvegardés et nouvelle qualification :** commit **`3f52ee4`**, poussé uniquement sur `origin/work/linux-arm64-021`. Run [`40062931635`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40062931635), source `3f52ee4`, cinq cibles natives. Attendre la fin du watcher et lire les reçus d'intégration, pas seulement les couleurs des jobs. Le README contenait déjà la distinction MSI/portable et la consultation 0.21 : il reste inchangé ; `docs/ARCHITECTURE.md` formalise maintenant le cycle et les frontières de propriété, sans les confondre avec une validation physique.

### Qualification `40062931635` et publication sûre du socket Unix

- Source **`3f52ee4`** : **Windows entièrement réussi**, y compris l'installation MSI et les huit vrais popups Explorer. Les reçus installés confirment un seul produit/raccourci, un owner et un HWND conservés entre lancements GUI/CLI/`.lnk` concurrents, aucun événement console, refus MSI en 154 ms sans connexion/staging ni modification des fichiers/enregistrements ; retrait coopératif et désinstallation réussis. Les 36 comparaisons d'icônes passent. Captures installée/réouverte examinées : typographie harmonisée et données indisponibles explicitement signalées. Le callback du tooltip recovered-active est réellement observé ; cette exécution n'a pas provoqué de déplacement de l'icône, donc ne prouve pas à elle seule la branche de suivi d'une relocalisation forcée.
- **macOS ARM entièrement réussi**, avec le transport d'ouverture indépendant du polling. **Intel échoue dans deux régressions de démarrage froid**, avant les builds release et la nouvelle preuve copied-app : le client voit le socket entre `bind(final)` et `chmod(0600)` et refuse correctement ses permissions transitoires. Bilan Rust Intel : 74 réussis, deux échoués, un ignoré. Ne pas présenter les scénarios natifs Intel non exécutés comme réussis.
- **Linux x64/ARM** : lint, tests, builds et ELF passent ; les vrais producteurs Gio et leur refus de cible absente passent maintenant. Le lancement s'arrête ensuite sur le garde de longueur : la fixture construisait un endpoint de **127 octets**, au-delà de la limite native retenue **<104**. Le pilote garde intégralement son HOME et son exécutable adversariaux, mais utilise un XDG_DATA_HOME privé de **89 octets pour l'endpoint**, contenant toujours Unicode, pourcentage, guillemets et antislash. Le refus d'un namespace étranger utilise maintenant un **autre config_dir sur le même endpoint**, au lieu d'un data_dir trop long/absent qui pouvait produire un faux positif de sécurité.
- **Correctif Unix commun implémenté** : staging `.sXXXXXX` privé 0700, socket préparé 0600 et non bloquant avant publication exclusive atomique (`renameat2(RENAME_NOREPLACE)` / `renamex_np(RENAME_EXCL)`). Aucun changement du umask global, aucune relaxation/reprise d'un socket dangereux, aucun écrasement de destination apparue pendant le démarrage. Le staging est plus court que le nom final, et son nettoyage comme celui du socket publié vérifie les inodes possédés.
- Nouvelles régressions : vrai consommateur connecté seulement après publication sécurisée, refus d'une destination étrangère et conservation d'un socket de remplacement lors du cleanup. La barrière macOS suspend désormais la vraie publication exclusive d'un socket déjà 0600, toujours avec lease réellement détenue et endpoint final non découvrable. Ce n'est pas un socket simulé ni un résultat de syscall remplacé.
- AST des pilotes Linux/macOS et formatage Rust exécutés après intégration. Prochain contrôle : **quatre cibles Unix seulement** ; Windows reste qualifié par `40062931635`, ses sources et pilotes étant inchangés. Le runner MSI était `runneradmin` avec rôle administrateur : ni qualification d'un token standard, ni regroupement taskbar manuel, ni matériel physique ne sont déduits de cette réussite. Aucun changement du PC installé, du README, de `main`, du site ou des releases.

**Point de reprise sauvegardé :** `4931e00` poussé sur `origin/work/linux-arm64-021` ; qualification Unix seule [`40065426316`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40065426316), source `4931e00`, `native-only=true`, `native-platform=all`. Aucun job Windows relancé : sa qualification complète `40062931635` reste valable sur les sources Windows inchangées. Attendre le watcher et les véritables reçus Gio/AX avant de clôturer ; aucun test physique ARM annoncé.

### Résultats Unix `40065426316` et identité GTK

- Source **`4931e00`** : **macOS Intel et Apple Silicon entièrement réussis**, y compris les régressions de publication privée, l'ouverture Finder à froid, la fenêtre AppKit accessible/retenue et le retrait normal. Les deux vrais `macos-app/app-smoke.json` indiquent `status=passed`, sans capture physique ni nouvelle autorisation OS. Windows reste qualifié par `40062931635` sur ses sources inchangées.
- **Linux x64 et ARM64** passent compilation, lint, tests et contrôles ELF, puis ouvrent réellement la consultation installée via Gio. L'échec suivant est identique dans les deux artefacts `wm-class.txt` : `com.roman-cuisset.miccamwatch`, **`Com.roman-cuisset.miccamwatch`**, alors que le lanceur déclare `StartupWMClass=MicCamWatch`. Le contrôle n'est pas réépinglé sur cette valeur incorrecte.
- **Correctif GTK** : conserver le nom de programme avant `gtk_init_check`, mais appliquer `gdk_set_program_class("MicCamWatch")` **après** cette initialisation et avant la première fenêtre. L'initialisation GDK remplaçait la classe préalablement définie par sa valeur dérivée/capitalisée ; la classe du lanceur doit rester celle de la vraie consultation.
- **Pilote de fermeture corrigé par inspection** : [`xdotool windowclose`](https://github.com/jordansissel/xdotool/blob/master/xdo.c) utilise `XDestroyWindow`, pas la demande normale du gestionnaire de fenêtres. Le scénario utilise désormais Alt+F4 dans son Xvfb privé, après activation et vérification du focus de la consultation possédée ; les assertions de masquage/réouverture du même XID et du même owner restent intactes. Ne pas présenter cette commande située après l'échec WM_CLASS comme un échec déjà observé en CI.
- AST Python et formatage Rust réussis après ces deux changements. Prochaine qualification : **Linux x64/ARM64 uniquement** ; les succès Windows/macOS ne sont pas relancés inutilement. Arbre suivi propre avant reprise à `e1f1a0f`, cinq exclusions et autres worktrees intacts ; README, site/confidentialité, `main`, tags et releases inchangés.

**Qualification Linux ciblée en cours :** correctifs **`519dc1c`** sauvegardés sur la seule branche de développement ; run [`40078682137`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40078682137), source `519dc1c`, `native-only=true`, `native-platform=linux`. Attendre ses deux reçus installés et ses captures avant de clôturer. Les captures GTK installée et AppKit clair/sombre/minimum du run précédent ont été réellement examinées ; les données AppKit marquées `SIMULATED` ne sont pas une preuve matérielle.

### Qualification finale — expérience applicative unifiée

Les échecs de ce lot sont clôturés par les preuves natives ci-dessous, sans réépingler WM_CLASS, simuler une ouverture ou assouplir l'authentification. Les sources des plateformes déjà réussies sont inchangées dans les commits suivants.

| Cibles | Source fonctionnelle qualifiée | Qualification native complète |
| --- | --- | --- |
| Windows | `3f52ee4` | [`40062931635`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40062931635), MSI/CLI/GUI/`.lnk`, HWND retenu, Shell, update portable et refus de double propriété MSI |
| macOS Intel / Apple Silicon | `4931e00` | [`40065426316`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40065426316), publication sécurisée, AppKit, Finder à froid, fenêtre retenue, bundle/LaunchAgent et retrait normal |
| Linux x64 / ARM64 | `519dc1c` | [`40078682137`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40078682137), GTK/Gio installés, ELF, frontières de privilèges, profils et menu natif |

- **Linux réellement confirmé après correction** : les deux `installed-launcher/results.json` et `wm-class.txt` ont été lus. Classe exacte `MicCamWatch`, même owner et même XID après activation répétée, Alt+F4, masquage et réouverture ; démarrage de fond caché et ouverture concurrente préservée. Les stderr confirment les véritables refus de namespace et d'asset modifié, pas un chemin absent/trop long. Retrait via la CLI et l'installateur réellement réussi, fichiers utilisateur conservés.
- **Couverture GTK** : 21 cas réussis **par architecture**, sept langues (`de/en/es/fr/ja/ru/zh`) et Adwaita/Adwaita sombre/HighContrast ; aucun cas en échec. Captures installée ARM et réglages russes sombres au minimum réellement examinées. Sur x64, le reçu identifie la vraie migration du producteur 0.20 ; sur ARM, il reste `null`, puisqu'aucune ancienne installation ARM publique n'est inventée.
- **Sauvegarde** : correctifs fonctionnels `29e0f32`, `3f52ee4`, `4931e00`, `519dc1c` et checkpoints documentaires conservés sur `work/linux-arm64-021`. La présente clôture ne modifie que la documentation. Comparaison finale nulle avec la base `a29dcc5` pour README/site/PRIVACY ; `origin/main` reste `dc40e95bc9c1c6a14386eb5e773f01b62b47aa71`. Aucune release, aucun tag, déploiement public ou remplacement de l'installation du PC.

**Limites et prochain point de reprise :**

1. Ne pas refaire ces qualifications sur les sources inchangées. Vérifications manuelles restantes : Windows 10/DPI multiples, lecteur d'écran, regroupement réel taskbar/Dock et installation MSI avec token utilisateur standard ; le runner Windows était administrateur.
2. Linux ARM64 est qualifié nativement sur runner hébergé, **pas sur Raspberry Pi ou autre appareil physique**. Wayland réel, périphériques USB/micro/caméra, sessions/login/reboot et permissions matérielles restent des validations distinctes ; les données GUI simulées et l'inventaire vide ne prouvent pas une protection.
3. Le PC conserve ses deux racines Cargo/MSI observées ; aucune consolidation automatique. Choisir une installation canonique puis restaurer/quitter normalement avant un retrait explicitement autorisé, sans supprimer une copie ou voler un owner en cours.
4. Les réserves de distribution restent inchangées : `VERSIONINFO` absent des EXE constatés, signature Windows/Developer ID macOS seulement avec de vrais certificats ; aucun contournement SmartScreen/Gatekeeper. Le refus de self-update MSI n'est pas un moteur de mise à jour MSI ni une preuve de désinstallation forcée avec protections actives.
5. Prochain lot demandé : réorganisation du README, **non commencée ici**. Une intégration ultérieure de `main` doit conserver les corrections SEO/confidentialité et les deux historiques de ROADMAP. Ne pas publier 0.21.0 ni pousser `main` sans nouvel accord.

## 2026-10-24 — Réorganisation documentaire : README et guides

### Isolation et versions

- Branche **`docs/readme-020-021`**, nouveau worktree `mcw-readme-docs-20261024`, créée depuis `origin/main` **`dc40e95`** : elle n'embarque aucun commit applicatif GUI/ARM64. Le worktree original reste sur **`work/linux-arm64-021` à `cafd98d`** ; ses cinq éléments non suivis et les autres worktrees ne sont ni déplacés ni nettoyés.
- **Public = 0.20.0**, source `a26522028eafe02591f58c03ceb5c367f2d5d60b`, neuf assets réellement listés par l'API ; **0.21.0 = candidat non publié**, dont les preuves sont référencées par commits/runs immuables dans `VALIDATION.md`. Le nouveau texte ne transforme ni ARM64 hébergé en certification physique, ni les nouvelles fenêtres Windows/Linux en fonctions du téléchargement stable.
- Le README de la base publique passe de **1 142 à 230 lignes**. Titre, badges, liens officiels, fonctionnalités et deux captures conservés ; les captures sont explicitement identifiées comme de véritables vues Windows **0.17.1**, pas des fenêtres 0.21 ni une preuve matérielle.

### Répartition sans perte des précautions et preuves

| Document | Rôle |
| --- | --- |
| `README.md` | Présentation, canaux de version, plateformes, démarrage sûr, commandes courantes, limites essentielles et navigation |
| `docs/README.md` | Index par besoin utilisateur/développeur, références et journaux historiques clairement séparés |
| `docs/INSTALLATION.md` | Assets 0.20, vérification, MSI/ZIP/DMG/Unix, privilèges, mises à jour, retrait, anciennes migrations et builds |
| `docs/USAGE.md` | Commandes, interfaces, profils, paramètres, limites, politique/enforcement, JSON/historique/confidentialité et dépannage |
| `docs/VALIDATION.md` | Matrices publique/candidate, CI versus matériel, sources et limites de reproduction |
| `docs/RELEASE_HISTORY.md` | Historique chronologique, liens de preuve immuables et table de correspondance des anciennes sections |
| `docs/WEBSITE.md` | Build statique, langues/canoniques, configuration Cloudflare et déploiement seulement après accord |

- Les **43 URL distinctes de release/run/advisory** de l'ancien README restent accessibles dans l'ensemble documentaire. Mesures détaillées et contrats déjà canoniques dans `ARCHITECTURE.md`/`SIGNING.md` sont référencés plutôt que recopiés ; ces fichiers et le journal historique Linux/macOS restent inchangés.
- Corrections d'instructions : `--add-path` est bien valide en 0.20 ; le défaut était de demander le checksum de `mcw-install.sh` dans un manifeste qui ne couvre pas ce script. Source d'installateur immuable, vérification des payloads séparée, pas d'endpoint alternatif ancien recommandé, blocage global Windows distingué du contrôle USB Linux ponctuel, profil de confidentialité distinct du profil TOML, rotation et absence d'expiration par âge explicites. Les commandes destructrices sont une référence en tableau, pas un bloc à coller intégralement.

### Vérifications et point de reprise

- **Rendu GitHub GFM réel des sept documents d'entrée/guides réussi**, 209 liens/ressources relatifs et ancres résolus, couverture des 43 anciennes preuves conservée. Audit HTTP de 115 destinations publiques ; l'ancien lien Ubuntu `packages.ubuntu.com` répond 500 en HEAD et GET, remplacé dans le guide courant par la source officielle Launchpad vérifiée 200, sans changer le nom ni le prérequis du paquet.
- **Smoke du vrai CLI public 0.20.0**, ZIP vérifié contre digest API et entrée exacte `SHA256SUMS` (`add9b3dc…d11579e`), seul `mcw.exe` extrait dans un périmètre temporaire : version, neuf surfaces d'aide et véritable validation du TOML documenté passent. APPDATA/LOCALAPPDATA privés ; aucun monitoring/tray/contrôle, update, uninstall ou autostart exécuté. `sh installer/install.sh --help` confirme réellement les options sans installation.
- Premier commit documentaire **`abd0468080f08b700efb713b8f077fd59174b5f4`**, poussé uniquement vers `origin/docs/readme-020-021` avec `[skip ci]` ; aucune pull request créée. La présentation de la branche publique `main` reste inchangée en attendant une autorisation d'intégration.
- **Surface GitHub réellement affichée et examinée** : README en desktop 1280 px sombre et mobile 390 px sombre/clair ; les sept images (cinq badges et deux captures) chargent. La séparation stable/candidat reste lisible sur mobile, sans débordement horizontal global observé du README, de l'index et du guide du site. Navigation réelle par les liens HTML : installation → usage → validation → historique → index → guide du site → retour au README. Pages accessibles sans connexion dans le navigateur de contrôle.
- Aucun build/test applicatif ou matériel relancé pour ce lot documentaire ; aucune modification source, installateur, site, confidentialité ou release. Les réponses HTTP sont un constat de disponibilité à cette date, pas une garantie future. Prochaine étape : revue de cette branche documentaire, puis intégration ciblée seulement après accord, en conservant les historiques ROADMAP des deux branches. **Aucune fusion `main`, aucun tag, aucune release et aucun déploiement Cloudflare autorisés par ce travail.**

## 2026-10-24 — Candidat d'intégration 0.21.0 : qualification avant publication

### Base, isolation et résolution des conflits

- Nouvelle branche **`release/0.21.0-candidate`**, worktree isolé `mcw-release-021-candidate-20261024`, créée depuis le dernier `origin/main` vérifié : **`dc40e95bc9c1c6a14386eb5e773f01b62b47aa71`**. Les HEAD distants GUI **`cafd98d`** et documentation **`5ee9d49`** sont synchronisés au départ ; aucun travail suivi non commité à sauvegarder, cinq exclusions historiques et stash conservés.
- Fusion GUI/ARM64 **`6a775f9`**, puis fusion documentaire **`89016c3`**, avec leurs parents conservés. Seuls conflits : deux blocs du README et un ajout terminal de ROADMAP. README repris exactement de la branche documentaire (230 lignes) ; union chronologique des ajouts GUI/ARM64 puis documentation. Les anciennes introductions 0.20 remplacées antérieurement par le chantier GUI restent dans l'historique Git ; elles ne sont pas réintroduites comme un état actuel.
- Contrôles d'intégration exécutés : sources `src/`, `native/`, installateurs, Cargo et workflows identiques à la branche GUI après les deux fusions ; `site/` et `PRIVACY.md` identiques à `origin/main`. Les deux nouveaux états de reprise documentaires sont intégralement conservés. Les autres worktrees et branches restent inchangés.
- La consigne antérieure de ne pas rejouer les sources inchangées est remplacée, pour ce lot, par la demande explicite d'une **qualification complète des cinq cibles sur le même candidat intégré**. Les succès historiques par plateforme restent des preuves distinctes, pas la preuve de ce nouveau commit.

### Contrôles et autorisations

- Version Cargo candidate **0.21.0** ; version publique du site toujours **0.20.0**. Révisions de documentation et préparation des notes à intégrer avant de figer le commit qualifié ; aucune nouvelle refonte des fenêtres.
- Relecture répartie sans écritures concurrentes : contrats Windows, GTK/ARM64, AppKit, site/SEO ; guides et notes de publication ont des propriétaires de fichiers séparés. Compilation, tests et formatage ne sont pas lancés pendant leurs éditions.
- Métadonnée de dépôt lue, sans modification : **`WINDOWS_RELEASE_APPROVED=true`**. Ce drapeau n'autorise pas cette session à publier, ne prouve pas un certificat de signature et ne remplace aucun résultat de qualification.
- **État à cet instant : qualification candidate non encore exécutée.** Attendre les vrais logs/reçus Windows, Linux x64/ARM64 et macOS Intel/ARM, puis distinguer les succès complets, preuves partielles et validations matérielles absentes. Aucun Raspberry Pi/SBC réel, nouveau consentement système ou matériel physique n'est disponible dans cette intégration.
- Autorisé : commits/push de la seule branche candidate et qualification CI. **Interdit sans nouvel accord : push/fusion `main`, tag `v0.21.0`, création de release GitHub et déploiement du site.**

### Correctif démontré : export du site public depuis un checkout candidat

- Première exécution : les cinq tests `node --test site/localization.test.mjs` réussissent ; l'export refuse le checksum de l'installateur 0.21 dans un site encore épinglé à 0.20. Le garde ne doit pas être supprimé et les pins publics ne doivent pas être avancés pour contourner ce refus.
- `site/build.mjs` conserve le fichier local uniquement si son SHA-256 correspond au pin ; sinon il charge l'installateur du commit public exact `site/release.json.source` sur GitHub HTTPS, sans redirection, avec échéance de 30 s, puis impose le même checksum avant tout remplacement de sortie. Aucun changement des templates, traductions, métadonnées publiques ou du contenu juridique ; ce chemin de build nécessite le réseau lorsque les sources de développement diffèrent.
- **Smoke après correction réussi** : export 0.20.0 ; les **45 fichiers générés**, dont huit pages de confidentialité et `install.sh`, sont identiques octet pour octet à un export indépendant du dernier `origin/main` extrait dans une fixture temporaire. SHA-256 du script publié inchangé : `b7348c9a6f37a3d4f89faa2b767fd4880162d90c8f5d7c151185e0823384d1bb`.
- Régression de refus exercée par une fixture jetable avec pin volontairement faux et véritable téléchargement : sortie 1 pour checksum incorrect, les 45 fichiers de sortie préexistants restent inchangés. Pas de faux transport ni de test réseau permanent non déterministe ajouté.
- **HTTP public réel** : seize accueils/alias et routes de confidentialité contrôlés sans authentification, tous 200 ; canoniques, huit alternates, nom MicCamWatch et objets WebSite cohérents, contacts mailto présents dans chaque politique, téléchargements toujours v0.20.0. Différence préexistante `zh-Hans` dans le HTML / `zh` dans le sitemap conservée, non introduite par l'intégration et sans changement de langues. Aucun déploiement effectué.

### Résultats du candidat intégré — source exacte et reçus

- Source figée **`4b90fd6bdfc2aaddf6218a64ce71f6554e06c789`**, poussée uniquement sur `origin/release/0.21.0-candidate`. Commits d'intégration `6a775f9` / `89016c3`, correctif d'export `f30076b`, préparation documentaire `4b90fd6`. Les commits d'évidence suivants ne modifient ni produit, ni build, ni installateur ; ils ne sont pas présentés comme le SHA exécuté par cette CI.
- **CI complète [`40106706806`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40106706806) réussie sur ce même SHA**, cinq jobs natifs, journaux et artefacts téléchargés puis examinés. Ce résultat remplace le besoin d'une preuve commune sans réécrire les échecs historiques des runs antérieurs.

| Cible | Tests Rust bibliothèque + CLI | Qualification native observée |
| --- | --- | --- |
| Windows x64 | 185 + 1 réussis, 6 ignorés, avec et sans tray ; scénarios ignorés ciblés exécutés séparément | Win32, MSI installé, Shell, profils, portable update, owners/IPC, ressources |
| Linux x64 | 105 + 1 réussis, 3 ignorés ; roundtrip SPA C réussi | GTK/Gio, installateur/frontière root, PipeWire virtuel, ELF62 |
| Linux ARM64 | 105 + 1 réussis, 3 ignorés ; roundtrip SPA C réussi | Même qualification native hébergée, ELF183 sur les deux binaires ; pas de Pi physique |
| macOS Intel | 79 + 1 réussis, 1 ignoré | AppKit, véritable DMG copié/Finder, fenêtre retenue, LaunchAgent et nettoyage |
| macOS Apple Silicon | 79 + 1 réussis, 1 ignoré | Même couverture ; `app-smoke.json` complet `passed`, pas une preuve partielle exit77 |

- **Windows installé** : un produit et un raccourci avec `--open`, AUMID commun, propriétaire et HWND conservés après démarrages GUI/CLI/`.lnk` séquentiels et concurrents, fermeture masquante et fond caché, aucun événement console. Refus réel de self-update MSI : code 2 en **142 ms**, zéro connexion tripwire et zéro staging ; payload, enregistrement, raccourci, PATH et données inchangés. Sortie coopérative et nettoyage de fixture réussis. Le runner était élevé ; ce n'est pas une preuve de compte standard.
- **GUI et provenance** : 62 captures Win32/sept langues/focus réel ; 21 cas GTK par architecture (sept langues × trois thèmes) ; 1 098 métadonnées de captures de vues AppKit par architecture, composites natifs montés et observations simulées, pas captures WindowServer ni matériel. Revue effective de fenêtres MSI initiale/réouverte, réglages russes Win32 au minimum, GTK x64 installé/ARM cold-launch et réglages russes sombres, AppKit clair/sombre/diagnostics Unicode au minimum.
- **Limite de capture GTK découverte** : `linux-aarch64/linux-desktop/installed-launcher/explicit-launch.png` est un premier cadre noir ; `cold-launch-race.png` et les vues GTK suivantes sont rendus. Le pilote capture après découverte XID/owner, sans preuve indépendante de premier dessin (lignes 213/229 de `native_linux_desktop.py`). Ne pas présenter cette image noire comme validation visuelle ; cela ne démontre pas à lui seul un défaut de rendu persistant du produit. Aucune temporisation arbitraire ni assertion assouplie ajoutée.
- **ARM et paquets** : hashes des quatre ELF extraits réellement recalculés et identiques aux reçus ; class64/little-endian/machines62/183 confirmés. Les deux véritables DMG CI sont conservés et hashés. Digests précis dans `docs/RELEASE_0.21.0.md`, explicitement **non publiés et non substituables au futur manifeste de release**. Les scénarios de migration d'un ancien producteur/format ne deviennent pas une preuve HTTPS de release0.21 inexistante.
- **Ressources** : scénarios fonctionnels candidats réussis ; cible souple 15 MB toujours non globalement atteinte, notamment tray Windows/macOS. Le `watch` Windows survit à ses 20 s de mesure puis est terminé par son identité retenue ; son code 1 n'est pas une sortie spontanée de démarrage. Mesures VM/échantillonnage ne prouvent ni tous les pics ni l'audibilité/capture physique.

### Désinstallation : quatre Unix réussis, Windows standard encore bloqué

- Workflow distinct [`40106709656`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40106709656), même source `4b90fd6` : **Linux x64/ARM64 et macOS Intel/ARM réussis**. Reçus `status=passed`, données conservées, retrait géré/Cargo réel, refus de payload modifié/liens/état de restauration inconnu/protection demandée/autostart étranger ; cas supplémentaires caméra et réservation réactive macOS réussis.
- **Windows échoue réellement** : nouveau compte ordinaire authentique, token non élevé, profil Windows chargé ; retrait différé de la paire sans frontend réussi, puis démarrage compagnon et diagnostic CLI terminés avec code 2, message `failed to register system tray icon`. Le workflow n'a donc **pas exécuté** les cas MSI/Cargo situés après ce point. Il n'est ni ignoré ni transformé en succès.
- Le reçu `private-launcher.bootstrap-result.json` indique un véritable Explorer, mais station **`Service-…`**, pas **`WinSta0`**. [Microsoft précise que seule WinSta0 est interactive](https://learn.microsoft.com/en-us/windows/win32/winstation/window-stations) ; ce prérequis du scénario GUI n'est pas satisfait. La cause interne exacte du refus Shell n'est pas prouvée ; ne pas conclure sans preuve à un bug produit, une erreur d'autorisations caméra ou un timeout à augmenter.
- **Prérequis manquant pour clore ce lot** : bureau jetable réellement interactif avec compte Windows standard authentique, puis exécution complète du même scénario avec tray vivant et cas MSI/Cargo. Le launcher existant `TASK_LOGON_PASSWORD` fournit ici une station de service ; la réussite sous `runneradmin` ne remplace pas cette preuve. Ne pas falsifier le marqueur hébergé sur le PC, modifier les ACL/UAC ou arrêter son tray pour contourner ce manque.

### État de publication et prochain point de reprise

- **NOT READY FOR RELEASE** : la CI native commune est réussie, mais le contrôle Windows ordinaire obligatoire reste en échec. Windows10/DPI multiples/lecteurs d'écran, Wayland/login réel, autorisations/macOS et matériel ARM/USB demeurent des périmètres non qualifiés ; ne pas transformer les runners en certification physique. Les vérifications matérielles absentes limitent les revendications, sans inventer de résultat ni de promesse universelle.
- README **231 lignes**, guides et notes à jour : source testée exacte, distinction public0.20/candidat0.21, workflow rouge explicite et plan ordonné/conditions d'arrêt. Le corps des futures notes de release est enrichi ; comparaison YAML confirme qu'aucun trigger, gate d'approbation/signature/provenance/publication n'a changé. Aucun workflow Release n'a été déclenché ; dix assets attendus, mais aucun manifeste/provenance final0.21 fabriqué.
- Reprendre par le prérequis Windows interactif, sans refaire les fenêtres ni intégrer arbitrairement des branches. Après résolution, qualifier les chemins affectés, revoir les reçus, fixer la révision réellement destinée à la publication et demander l'accord distinct avant `main`, tag, release et site. Les procédures et limites détaillées sont dans `docs/VALIDATION.md` et `docs/RELEASE_0.21.0.md`.

## 2026-10-24 — Windows standard interactif : reprise et sauvegarde continue

### Base protégée et diagnostic retrouvé

- Reprise sur `release/0.21.0-candidate` à **`4d6284d`**, distant synchronisé ; produit qualifié inchangé depuis `4b90fd6`. `main=dc40e95`, GUI=`cafd98d`, documentation=`5ee9d49`, autres worktrees et cinq exclusions historiques conservés. Aucun changement de l'installation, du microphone ou de la caméra du PC personnel.
- L'échec `40106709656` n'est pas rejoué pour le confirmer : reçus existants relus. Le compte non administrateur et son profil sont authentiques, mais `TASK_LOGON_PASSWORD` impose ici session0/station `Service-0x0-46cca9$`. Un Explorer dans cette station ne prouve pas un bureau interactif ; le refus `Shell_NotifyIconW(NIM_ADD)` est réel, sans cause interne Win32 additionnelle établie.
- Recherche Microsoft : `LOGON_WITH_PROFILE` charge un profil, pas une session graphique indépendante. `CreateProcessWithLogonW` exige aussi l'accès à la station/au desktop ; changer le SessionId d'un token ou élargir des ACL ne doit pas devenir une fausse preuve de connexion standard. Préférer une véritable connexion console/RDP d'un compte standard dans une VM jetable. L'ancien essai RDP ayant déjà échoué à établir un desktop, pas de boucle de relance du batch ni d'hypothèse RDP réussie sans nouvelle preuve.
- Inventaire local strictement en lecture seule : aucun hôte SSH configuré ; aucun `Get-VM`, VBoxManage, vmrun ou service Hyper-V de gestion retourné. Le PC est un iRU 15ALG avec hyperviseur détecté : cela ne fournit pas une VM Windows jetable préconfigurée et n'autorise pas à activer des composants, créer des comptes ou changer ses protections.

### Point de sauvegarde WIP — contrôles MSI standard

- Modifications de l'agent `StandardApplicationProof` **déjà écrites sur disque**, limitées à `installer/tests/native_uninstall.py` et `installer/tests/windows-application-native.ps1`. Le parent sauvegarde ce lot avant toute opération longue ; aucune dépendance à une dernière sauvegarde en fin de session.
- Le cas MSI doit maintenant réutiliser son unique installation et son vrai raccourci pour exécuter la preuve existante avec **`-RequireStandardUser`**, puis exiger `passed`, `cleanupSafe` et `standardUserQualified` avant délégation/retrait. Contrôles de produit étranger, données conservées et PATH préexistant maintenus ; une erreur conserve les preuves et l'installation de fixture au lieu de forcer son retrait.
- Le nouveau mode doit vérifier le véritable compte local standard, token primaire non restreint/non élevé sans appartenance administrateur, profil réellement chargé, logon interactif, sessions token/processus/LSA cohérentes et non nulles, station `WinSta0` et desktop courant égal au desktop d'entrée. Un token administrateur filtré ou déplacé ne doit pas être présenté comme un compte standard connecté.
- **État à cette sauvegarde : code de preuve WIP, ni parsé, ni compilé, ni exécuté.** Les gardes GitHub-hosted d'origine ne sont pas affaiblis ; aucun faux marqueur, aucune extension implicite aux VM externes. Il reste à relire/vérifier ce lot puis fournir une voie reproductible et strictement isolée vers un vrai contexte interactif, ou documenter précisément le prérequis non disponible.
- Aucun nouveau résultat natif, aucune clôture du blocage et aucune autorisation de publication déduits de cette sauvegarde. Ne pas relancer les cibles Unix inchangées. Ne pas fusionner `main`, créer de tag/release ou déployer le site.

### Sauvegarde suivante — vraie VM séparée et contrôles partagés

- Premier point WIP publié : **`d2a2b75`**, uniquement branche candidate. Relecture spécialisée de ses deux scripts : aucun défaut concret trouvé ; aucune validation d'exécution déduite de cette relecture.
- Inventaire GitHub réellement consulté : **zéro runner auto-hébergé enregistré**. Il n'existe donc pas de session standard jetable disponible à laquelle déléguer honnêtement le test interactif. Ne pas lancer un job en attente sans hôte ni réessayer le même batch/RDP déjà refusé.
- Code de contexte standard déplacé dans `installer/tests/windows-fixture-context.ps1`, réutilisé par le consommateur MSI et `native_uninstall.py` avant toute isolation AppData. Ajouts : station visible, desktop d'entrée accessible, vrai Explorer du même SID/session et non élevé, admission explicite d'une VM reconnue exécutant un runner éphémère interactif. Le nom de compte est dérivé du **token réel**, pas de `USERNAME` modifiable ; aucune élévation, modification d'ACL/token/session ou action SDK.
- `windows-standard-user.ps1`, `.github/workflows/windows-standard-user.yml` et `docs/WINDOWS_STANDARD_USER.md` écrits : compte local standard réellement connecté, hôte jetable dédié, aucun produit/données préexistants, services préparés dans le guest, compilation verrouillée, inventaire positif zéro capture/caméra, WiX épinglé, vrais cas portable/MSI/Cargo, échec conservé. Ce scénario **n'a pas été exécuté** et ne prouve pas qu'une VM existe.
- `windows-context-smoke.ps1` prépare uniquement parsing PowerShell, compilation inerte du C# embarqué, observation du contexte réel et refus de contexte non autorisé ; son reçu distingue explicitement `qualificationStatus=not_executed`. Aucun exécutable MicCamWatch ni MSI n'y est lancé.
- Le workflow `native-uninstall.yml` existant offre deux sous-ensembles manuels : `windows-context` (lecture seule) et `windows-standard` (appel du scénario VM avec acquittement explicite). La voie réutilisable évite de devoir publier le nouveau workflow sur `main` pour l'enregistrer ; aucun changement sur `main` n'est autorisé par cette préparation. Les quatre cibles Unix déjà validées ne seront pas relancées.
- **État de ce checkpoint : nouvelles sources encore non parsées/non compilées/non exécutées.** Prochaine action : smoke en lecture seule local, vérifications syntaxiques, puis même smoke sur runner hébergé ; ensuite conserver les preuves. Le test GUI/MSI standard complet reste bloqué par l'absence de véritable VM/runner et conserve le verdict **NOT READY FOR RELEASE**. Sauvegarder ce point avant toute exécution longue.

### Corrections de l'outillage et smoke local réel

- Deuxième sauvegarde WIP **`fa47c64`** poussée avant les essais ; neuf fichiers de qualification/documentation seulement, vérification des chemins ajoutés et motifs courants de secrets sans correspondance. Sources applicatives, site, MSI de production et autres worktrees exclus.
- Première exécution du smoke local : quatre scripts parsés et trois blocs C# compilés, puis échec de l'outillage parce qu'il essayait de compiler son propre appel `Add-Type` dynamique. Reçu conservé dans `%TEMP%/mcw-context-smoke-fa47c64`. Correction : parser le smoke lui-même sans recompilation récursive de son moteur.
- Inspection supplémentaire : le paramètre `RequireStandardUser` du helper dot-sourcé pouvait écraser celui du consommateur. Son paramètre CLI reçoit désormais un nom propre avec alias externe conservé ; un probe PowerShell jetable a confirmé que l'exigence vraie du consommateur reste vraie après dot-sourcing.
- Smoke corrigé réellement exécuté sous Windows PowerShell local : **réussi**, quatre scripts parsés, trois blocs C# réels compilés, observation native du token/profil/station/desktop/Explorer et refus du PC personnel/non standard. Reçu `%TEMP%/mcw-context-smoke-fa47c64-corrected` : `safetySmokeStatus=passed`, `qualificationStatus=not_executed`, `productActionsInvoked=false`. Aucun MCW, MSI, désinstalleur ou contrôle de périphérique lancé.
- Parsing AST Python réussi sans `__pycache__` et parsing YAML des deux workflows réussi. Vérification statique des événements manuels, labels VM et permissions `contents: read`. Cela ne valide pas l'exécution du MSI/GUI ; prochain contrôle : smoke lecture seule `windows-context` sur runner GitHub hébergé, après sauvegarde des corrections. Aucun nouveau test Unix ni build Rust produit nécessaire pour ces seuls changements d'outillage.

### Sauvegarde avant attente GitHub

- Corrections de portée PowerShell, smoke corrigé et documentation enregistrés/poussés dans **`5cc515b`** (`test(windows): validate real fixture context without product actions`). Le produit demeure inchangé depuis la source qualifiée `4b90fd6`.
- Sous-ensemble manuel **`windows-context`** réellement déclenché sur cette révision : [`40127113302`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40127113302). À ce checkpoint, son résultat n'a pas encore été inspecté ; ne pas le marquer réussi. Aucun dispatch `windows-standard` sans runner disponible.
- Reprise après interruption : consulter ce run existant, récupérer l'artefact `windows-context-smoke`, lire `result.json` et les observations natives avant toute conclusion. Il doit rester `qualificationStatus=not_executed` et `productActionsInvoked=false`. Ne pas redéclencher les cinq builds validés ni le batch/RDP historiquement inadapté.

### Résultat GitHub inspecté et point de reprise stable

- [`40127113302`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40127113302) **réussi**, source exacte d'outillage **`5cc515b4c65a76b50e70a1edc70f9c48565e8c8d`**. Le seul job exécuté est `windows_context` ; aucun nouveau build/test produit ni scénario natif Unix/MSI n'est déduit des jobs hors sous-ensemble.
- Journaux et artefact [`windows-context-smoke` / `12572758614`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40127113302/artifacts/12572758614) effectivement téléchargés et lus. Copie conservée hors dépôt dans `%TEMP%/mcw-021-context-proof-40127113302`. Quatre scripts parsés, trois blocs C# compilés ; reçu `safetySmokeStatus=passed`, **`qualificationStatus=not_executed`**, **`productActionsInvoked=false`**.
- Observations Win32 réelles du runner hébergé : sessions processus/token/LSA **2**, logon **2**, station **WinSta0 visible**, desktop courant/d'entrée **Default**, Explorer réel dans la même session ; mais token élevé, privilège de compte **2** et shell élevé. Le garde a correctement refusé ce contexte administrateur. Cela confirme qu'un bureau interactif du runner principal n'établit pas une connexion interactive du compte standard secondaire.
- Dépendance du consommateur partagé ajoutée aux chemins surveillés de `windows-brand.yml`, sans changement de job, de produit, de privilèges ni relance complète. Les liens locaux des quatre guides/ROADMAP concernés ont été contrôlés : **49 chemins valides**. Documentation de release et validation mises à jour, sans modifier README, GUI, ARM, installateurs de production ou site.
- Sauvegardes successives déjà poussées : **`d2a2b75`**, **`fa47c64`**, **`5cc515b`**, puis **`8be85f8`** avant l'attente du run. Les changements importants sont dans Git, pas uniquement dans une session d'agent. Les erreurs du smoke initial et leur correction restent documentées ; aucun résultat n'est réétiqueté en qualification standard réussie.
- **Prochaine action réelle** : un opérateur doit fournir une VM Windows jetable avec compte local `mcw_qa_*` réellement connecté et runner éphémère `mcw-standard-disposable-*` en ligne. Suivre `docs/WINDOWS_STANDARD_USER.md`, puis dispatcher `native-uninstall.yml` sur la révision candidate choisie avec `platform=windows-standard` et acquittement explicite ; inspecter les preuves natives et MSI complètes. Aucun runner disponible à cette sauvegarde : ce scénario n'a pas été lancé ni mis en attente artificiellement.
- **NOT READY FOR RELEASE** demeure le verdict : défaut produit non démontré par le refus du tray en station de service ; environnement de qualification interactive standard manquant. Pas de suppression de contrôles, de mutation du microphone/caméra ni de désinstallation sur le PC personnel. Pas de `main`, tag, release ou déploiement volontaire du site. Les preuves cinq cibles et les limites matérielles ARM existantes restent conservées.

## 2026-10-25 — Préparation autonome de la qualification Windows locale

### État repris et limites d'autorisation

- Reprise sur **`release/0.21.0-candidate` à `084e5f8`**, arbre propre et autres worktrees préservés. Les sources produit restent celles qualifiées sur `4b90fd6` ; ne pas relancer les cinq cibles déjà réussies pour de simples vérifications d'outillage.
- L'inventaire Hyper-V réellement élevé du 24 octobre a réussi avec jeton administrateur actif, `Get-VM`, `Get-VMHost` et `Get-VMSwitch` : **zéro VM enregistrée à cet instant**, commutateurs Default Switch et WSL existants laissés intacts. Ce résultat corrige l'ancien inventaire non élevé incomplet ; il n'autorise pas à conclure sur une liste actuelle inaccessible.
- Le contrôleur UAC borné à une seule nouvelle VM a ensuite été vérifié actif, sans créer de VM. Son reçu actuel est **`status=closed`, `ownedVmId=null`**, échéance `2026-10-25T03:13:16Z`. Les droits élevés ne sont donc plus disponibles pour cette absence. **Aucune nouvelle demande UAC, aucun contournement, aucun compte/paramètre de sécurité/logiciel/périphérique hôte modifié.**
- L'autorisation utilisateur permet uniquement une nouvelle VM jetable, jamais une VM ou un commutateur existant ; aucun redémarrage hôte ni changement des installations MicCamWatch réelles. La création/installation/test GUI restent conditionnés à des droits disponibles et aux conditions de licence/installation.

### Média Microsoft et prochain checkpoint

- La page Microsoft publique de téléchargement fournit un lien direct officiel x64 EN-US, vérifié par en-têtes HTTP sans connexion ni formulaire : `https://software-static.download.prss.microsoft.com/dbazure/26300.9457.260913-1737.26h2_ge_release_svc_refresh_CLIENTENTERPRISEEVAL_OEMRET_x64FRE_en-us.iso` ; taille annoncée **8 225 329 152 octets**.
- SHA-256 **publiée par Microsoft**, pas encore mesurée sur un fichier local à ce checkpoint : `BC3F24086EBADC94489066B5AD78089E2CF5C3491E90E790BB81A2B199C10E38`. Source : page de vérification d'authenticité liée depuis le Centre d'évaluation. Le téléchargement public n'est ni une inscription, ni une acceptation du contrat Windows, ni une activation.
- Préparation autorisée : récupérer uniquement ce média public sans cookies/authentification/soumission de formulaire, conserver un fichier partiel en cas d'interruption, puis vérifier digest, architecture et version avant toute utilisation. Si une interaction personnelle devient nécessaire, ne pas la contourner. Ne pas accepter automatiquement les conditions d'installation Windows.
- Deux revues indépendantes en lecture seule portent sur les fixtures Windows et sur les risques/gates de publication, à partir du commit `084e5f8` exact. Les scripts existants sont encore réservés aux vrais environnements GitHub : la voie locale doit être explicite et préserver profil réel, session interactive standard, Explorer, confinement des chemins, zéro périphérique de capture et conservation des données.
- **NOT READY FOR RELEASE** inchangé. Prochain travail atteignable sans UAC : vérification du média, contrôle/préparation reproductible de l'outillage local et documentation ; sauvegarder les résultats régulièrement. Aucun résultat VM/MSI/GUI non exécuté ne doit être transformé en succès.

### Checkpoint — voie locale explicite et refus du PC réel

- Sauvegarde précédente publiée avant les modifications : **`d7c9a89`**. Décision conservatrice sur le média : malgré l'accès HTTP public, Microsoft indique explicitement inscription puis téléchargement ; l'utilisateur interdit toute inscription/acceptation à sa place. **ISO non téléchargée**, aucune mesure locale de SHA/architecture/version, aucun contrat accepté. Le hash ci-dessus reste seulement une valeur publiée, pas une vérification de fichier.
- Voie **locale sans runner** ajoutée aux scripts de qualification existants : `windows-fixture-context.ps1`, `windows-standard-user.ps1`, `native_uninstall.py` et `windows-application-native.ps1`. Sélection explicite `LocalDisposableVm` + racine privée ; aucun faux `GITHUB_ACTIONS`, `RUNNER_ENVIRONMENT`, `RUNNER_TEMP` ou profil. Le mode runner conserve ses prérequis propres.
- Les gardes communs exigent VM reconnue réellement observée, compte local standard authentique `mcw_qa_*`, token/profil/WinSta0/Explorer réels et racine possédée sans reparse. L'orchestrateur local vérifie une révision complète fournie indépendamment, un checkout suivi inchangé et placé dans cette racine. Le consommateur natif borne source, payload, Cargo et preuves à la même racine avant toute action ; le même MSI et son vrai raccourci restent utilisés jusqu'au retrait. Inventaire positif zéro micro/caméra, refus de produits/données préexistants et restauration inchangés.
- Smoke local réellement exécuté : **réussi**, quatre scripts PowerShell parsés et trois blocs C# compilés ; le PC physique iRU est refusé par le nouveau mode local. Reçu `%TEMP%/mcw-context-local-20261025/result.json` : `safetySmokeStatus=passed`, `qualificationStatus=not_executed`, `productActionsInvoked=false`, `actualLocalHostAdmissionAccepted=false`.
- Trois entrées réellement exercées en refus sûr — orchestrateur, consommateur natif Python et consommateur application MSI — renvoient **1 avant toute création de dossier de preuve natif**, sans lancer MCW, msiexec ou un contrôle de périphérique. Journaux conservés dans `private-local-entry-refusals.json` du même dossier. Probe jetable supplémentaire : les paramètres stricts du caller sont conservés après dot-sourcing ; AST Python réussi sans cache.
- Deux revues spécialisées du commit `084e5f8` n'ont trouvé aucun défaut concret ; la relecture de l'intégration locale actuelle n'a pas trouvé de faille ou régression substantielle. **Ce sont des revues de source, pas des essais VM.** Aucun test positif standard/GUI/MSI local n'a été exécuté.
- `docs/WINDOWS_STANDARD_USER.md` documente les deux routes séparées, les commandes locales reproductibles, les preuves attendues, l'absence de fausse provenance GitHub, les prérequis légaux et l'arrêt de la seule nouvelle VM. Prochaine vérification : non-régression ciblée du consommateur MSI/GUI existant sur runner Windows hébergé, parce que son API de racine a changé ; pas de relance des quatre cibles Unix ou de la suite complète produit.

### Sauvegarde avant la non-régression ciblée

- Implémentation locale complète et documentation publiées sur la seule branche candidate dans **`84e3ed7`** (`test(windows): add explicit local disposable guest qualification`). Sept fichiers de qualification/guide/ROADMAP seulement ; différences, chemins et motifs courants de secrets contrôlés avant push, aucun binaire/cache/preuve privée ajouté.
- Workflow existant **Windows branding/application** réellement déclenché : [`40168712499`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40168712499), source `84e3ed7`. Il reconstruit les vrais binaires/MSI puis exerce l'ancien chemin installé, parce que l'API partagée de racine privée a changé. À ce checkpoint, **résultat non encore inspecté**. Ne pas relancer ce run ni les cinq builds par défaut après interruption.
- Reprise : consulter ce run existant, récupérer l'artefact `native-windows-brand-shell`, examiner les journaux et reçus du MSI/application/cleanup. Même une réussite sous administrateur hébergé ne fermerait pas le blocage de session standard locale. Conserver `NOT READY FOR RELEASE` tant que l'essai réel dans une nouvelle VM n'est pas fait.

### Résultat inspecté et arrêt au dernier point sûr

- [`40168712499`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40168712499) **réussi** sur la révision exacte **`84e3ed73a0d2beecb34aa40ee2918b8114b9e2d2`**. Logs de compilation, WiX 5.0.2 et scénarios natifs lus, pas seulement la couleur du job. Artefact [`native-windows-brand-shell` / `12589086891`](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40168712499/artifacts/12589086891) téléchargé dans `%TEMP%/mcw-021-brand-proof-40168712499`.
- Reçus MSI/application effectivement examinés : installation/retrait MSI **0**, portée per-user **AssignmentType=0**, une seule entrée/raccourci `--open` et AUMID cohérent, owner/HWND conservés à la fermeture/réouverture et aux activations concurrentes, PATH préexistant conservé, sortie coopérative. Refus réel de self-update MSI : **exit2, 165ms, zéro connexion proxy et aucun événement de staging**, payload/préférences/registre/raccourci inchangés.
- Capture native de la fenêtre réouverte affichée et examinée. Le reçu application précise **`status=passed`, `cleanupSafe=true`, `fixtureHostKind=github-hosted`, `requireStandardUser=false`, `standardUserQualified=false`**. Ce résultat ferme la non-régression du chemin runner existant, **pas** la qualification locale standard ni les périmètres matériels/Win10/DPI/langues/thèmes non réexercés.
- Documentation contrôlée : **50 chemins locaux valides**, ancre de la route locale présente ; **cinq exemples PowerShell parsés sans être exécutés**. Aucun nouveau test Rust complet, aucun rerun Unix, aucune modification du code produit, des installateurs de production, de README, du site/SEO ou de la confidentialité.
- Sauvegardes de cette reprise : **`d7c9a89`** (inventaire/autorité expirée), **`84e3ed7`** (voie locale complète + refus sûrs), **`7bf09da`** (référence du run avant attente), puis ce bilan documentaire. Aucun travail important d'intégration n'est laissé uniquement dans un agent ou en mémoire ; les preuves privées restent hors Git.
- **État réel final : ISO non téléchargée ; nouvelle VM non créée ; Windows non installé ; aucune session standard de test ouverte ; scénario local MSI/Cargo/portable complet non exécuté.** Le contrôleur est déjà fermé sans VM possédée : aucune VM n'a été arrêtée/supprimée à tort et aucune nouvelle UAC n'a été demandée pendant l'absence.
- **Reprise précise** : l'utilisateur doit compléter les démarches Microsoft et fournir un média/droit d'évaluation valide, puis être présent pour une nouvelle autorisation UAC bornée à une nouvelle VM. Vérifier ensuite le fichier ISO lui-même, configurer le guest sans partages/périphériques personnels, respecter les conditions Windows, se connecter réellement en compte standard et exécuter la commande locale du guide avec la révision candidate complète alors approuvée. Ne pas rejouer la qualification runner réussie ni substituer le PC réel à la VM.
- **NOT READY FOR RELEASE** : les obstacles restants sont les prérequis personnels/UAC et l'absence de preuve positive standard. Aucun contournement de sécurité, aucun changement des comptes/protections/installations MicCamWatch de l'hôte, aucune fusion `main`, aucun tag/release ni déploiement du site.

## 2026-10-25 — ISO fournie et reprise Hyper-V locale

- Reprise propre sur **`release/0.21.0-candidate` à `c10c651`** ; autres worktrees et historique GUI/ARM/docs préservés.
- L'utilisateur a fourni l'ISO Microsoft `26300.9457.260913-1737.26h2_ge_release_svc_refresh_CLIENTENTERPRISEEVAL_OEMRET_x64FRE_en-us (1).iso`. Mesure réelle : **8 225 329 152 octets**, fichier inchangé pendant le calcul ; SHA-256 **`BC3F24086EBADC94489066B5AD78089E2CF5C3491E90E790BB81A2B199C10E38`**.
- Concordance exacte avec la référence officielle **Enterprise Eval x64 EN-US, Windows 11 26H2**, page Microsoft **KB5130867** relue : <https://support.microsoft.com/en-us/servicing/os/windows/docs/2026/09/verify-the-authenticity-of-a-windows-11-enterprise-evaluation-iso-file>. Le suffixe `(1)` du téléchargement ne change pas les octets. Identité/integrité établies par le hash officiel ; inspection WIM montée en lecture seule prévue sous élévation, sans lancer Setup sur l'hôte.
- Ressources observées avant élévation : **32 Go installés environ, 12 processeurs logiques, 10,8 Gio de mémoire libre et 116,4 Gio de disque libre**. Prévision bornée : une seule nouvelle VM génération 2, **4 vCPU, mémoire dynamique 4/6/8 Gio, VHDX dynamique 80 Gio**, Secure Boot et TPM virtuel. Adapter le parallélisme de compilation dans le guest si nécessaire ; ne jamais libérer des ressources en arrêtant des processus personnels.
- Jeton actuel **non élevé** ; `Get-VM`, `Get-VMHost`, `Get-VMSwitch` refusent réellement l'autorisation. Nouveau contrôleur préparé, parsé et borné à une seule identité VM nouvelle ; il réutilise les contrôles précédents, autorise uniquement l'ISO exacte vérifiée et fournit écran/clavier virtuels propres au guest sans transfert de dossiers, presse-papiers ou capture physique. Ni UAC accordée ni création/installation/test standard ne sont encore revendiqués à ce checkpoint.
- L'autorisation actuelle permet une **nouvelle demande UAC**, mais aucune acceptation de licence/étape initiale à la place de l'utilisateur. Demander explicitement toute intervention ; ne jamais contourner Windows OOBE, les conditions d'évaluation ou les protections. **NOT READY FOR RELEASE** tant que la preuve interactive standard complète manque. Aucun changement `main`, release, tag ou site.

### Checkpoint — UAC renouvelée, ISO inspectée, VM créée

- Sauvegarde préalable publiée : **`65ccab7`**. Nouvelle UAC réellement accordée ; le contrôleur élevé possède un jeton administrateur actif et les trois commandes Hyper-V fonctionnent. Inventaire avant création : **zéro VM**, deux commutateurs existants conservés sans changement de configuration.
- Inspection temporaire de l'ISO en **lecture seule**, démontée ensuite : volume **UDF `CENA_X64FREE_EN-US_DV9`**, une image **`Windows 11 Enterprise Evaluation` / `EnterpriseEval`**, architecture **9 = x64**, version **`10.0.26300.9457`**, langue **`en-US`**. Signature de `setup.exe` **Valid**, signataire **Microsoft Corporation**. Aucun `setup.exe` lancé sur l'hôte.
- Seule VM créée : **`MCW-021-QA-c1c5de53`**, ID **`260b15b3-0281-4520-aa6b-7ca208a01d26`**. Configuration réellement relue : génération 2, 4 vCPU, mémoire dynamique **4/6/8 Gio**, un VHDX propre au guest sans passthrough de disque physique, ISO officielle exacte, TPM activé et Secure Boot **On** (l'enum Hyper-V sérialise On en **0**, vérifié via son type réel). Démarrage automatique `Nothing`, checkpoints automatiques désactivés.
- Preuves privées : `%TEMP%/mcw-021-eval-20261025-ngbjedk9/` contient contrôleur, manifeste ISO, inventaires, métadonnées WIM, configuration VM et captures réelles. Le contrôleur actif **PID11248**, échéance **2026-10-25T18:48:02Z**, garde exclusivement l'ID de cette VM ; ne pas relancer un autre contrôleur, créer une deuxième VM ou toucher aux VM préexistantes. Si ses droits expirent, demander explicitement une nouvelle autorisation adaptée à la VM déjà créée.
- Démarrage réel du DVD et écrans langue/clavier/options de Windows Setup observés par l'API de capture du **seul guest**. Entrées clavier envoyées à son périphérique virtuel ; aucun VMConnect/RDP, presse-papiers, dossier personnel, microphone ou caméra hôte redirigé. Aucun test produit standard exécuté à ce point, aucune licence acceptée par l'assistant.
- Prochaine étape : poursuivre uniquement l'installation sur le VHDX jetable, présenter les conditions Windows à l'utilisateur avant toute acceptation, puis provisionner les dépendances et un compte standard réel dans le guest. Conserver le verdict **NOT READY FOR RELEASE** et les sources produit inchangées.

### Point d'attente — conditions Windows affichées, non acceptées

- Navigation réelle du setup : langue/clavier EN-US, installation limitée au seul nouveau VHDX vide ; l'avertissement d'effacement concerne exclusivement ce disque virtuel vérifié. Arrivée sur **« Applicable notices and license terms »**, Microsoft Software License Terms / Windows Operating System, **Last updated April 2024**.
- **Aucune acceptation envoyée.** Capture `%TEMP%/mcw-021-eval-20261025-ngbjedk9/setup-license-unaccepted.png`. Le bouton Accept est présent mais l'assistant attend l'accord personnel explicite demandé par l'utilisateur. Aucun contournement de Setup/OOBE, aucun Windows installé, aucun compte standard créé et aucun scénario MCW positif exécuté à cette étape.
- Reprise après consentement uniquement via le contrôleur existant et l'ID VM enregistré ci-dessus ; vérifier son état/échéance avant toute action. Pendant l'attente, le guest est sur l'écran de licence ; ne pas confondre ce démarrage PE avec une installation achevée ou un test produit réussi.

### Checkpoint — consentement explicite et installation du guest démarrée

- L'utilisateur a répondu explicitement **« J'accepte pour cette VM de test »** à la demande de licence, avec autorisation de valider uniquement le guest. Consentement conservé dans `windows-license-consent.json` privé ; bouton Accept activé **après** cette réponse, jamais avant.
- Cible réelle affichée : **Disk 0 Unallocated Space, 80.0 GB**, unique nouveau VHDX vérifié. Récapitulatif observé : **Install Windows 11 Enterprise Evaluation / Keep nothing** ; installation démarrée et capture réelle **5%** conservée. Les redémarrages annoncés concernent seulement la VM, pas l'hôte.
- Deux sous-agents préparent, dans des fichiers privés séparés, le provisioning des outils du guest et le wrapper de qualification interactive. Aucun n'a le droit d'exécuter un installateur, gérer Hyper-V, modifier le produit ou publier ; l'intégration, le contrôle des scripts et toute exécution restent au parent. Toute licence additionnelle nécessitant consentement doit rester explicite.
- Windows reste **en cours d'installation** à ce checkpoint : ne pas annoncer un bureau standard, un build ou un test MCW réussi. Le contrôleur et la VM à reprendre sont toujours ceux de la section précédente ; aucune nouvelle VM à créer.

### Préparation des outils sans installation sur l'hôte

- L'utilisateur a donné un deuxième accord explicite **« Oui, uniquement dans cette VM »** pour installer Git, Python, Rust/Cargo, .NET, WiX et Microsoft Build Tools/MSVC/SDK avec leurs conditions applicables. Reçu privé `guest-tools-consent.json` ; l'accord n'autorise aucune installation ni modification de sécurité hôte.
- Contributions privées inspectées : `guest-prerequisites.ps1` (stades Machine/User séparés, licences par défaut refusées, sources officielles/signatures/hashes, aucun MCW installé) et `guest-qualification-launch.ps1` (compte standard console réel, SHA indépendante, admission existante, environnement VS pris en charge, journal immuable). Les **deux scripts ont réellement passé le parseur PowerShell**, mais aucun installateur d'outil ni test produit n'a encore été exécuté.
- Disque de données créé avec l'API Windows intégrée, sans installation de logiciel hôte : **106496 octets**, SHA-256 **`9FF1212A62FFF43B29B5294D7CF6D409446CB97CF57A13279CCF9DA7E9A2528B`**. Archive relue : les deux scripts, l'aide partagée inchangée et `SHA256.json`, rien d'autre. Aucun mot de passe, manifeste privé ou dossier personnel. Ce DVD supplémentaire est attaché uniquement à l'ID VM possédé ; aucun partage de dossiers/presse-papiers/périphériques.
- Le helper partagé transféré reste byte-identique, SHA-256 **`1ee6929cb938b29b5349534ecc70791fab72bd8f63a09e1230dfbdf00a71ec66`**. Les scripts doivent utiliser l'UUID firmware réellement lu par le canal PowerShell Direct lié à cette VM si celui-ci diffère de l'ID Hyper-V ; ne jamais falsifier les métadonnées.
- Correction documentaire démontrée : le manifeste WiX est **`dotnet-tools.json` à la racine**, pas `.config/dotnet-tools.json`. Guide corrigé ; pin 5.0.2/roll-forward inchangés, aucune installation .NET6 ajoutée. Le provisioning préparé vise .NET8, dont l'exécution réelle de WiX reste à observer dans le guest.
- Dernier écran observé : seconde phase d'installation Windows **42%** après son premier redémarrage. Ce pourcentage n'est pas une installation terminée, une activation ou une qualification standard réussie. Reprise : terminer OOBE sans contournement, puis outils Machine, vrai logon standard, outils User, snapshot propre et orchestration existante.

### Checkpoint — Windows installé, OOBE local et élévation souris annulée

- Windows a terminé sa copie/redémarrages et démarre réellement dans OOBE. Région et clavier EN-US configurés par les écrans normaux ; réseau NAT du seul guest opérationnel. Secure Boot/TPM et les deux DVD relus ; aucun périphérique physique ni partage redirigé. **Pas encore de bureau de provisioning ni de compte standard connecté.**
- OOBE Enterprise présente **Sign-in options → Domain join instead**, chemin intégré de création d'un compte local sans compte Microsoft personnel. Le focus réel de ce bouton est observé, mais Enter et Space n'ont pas fait avancer cet écran. Ne pas attribuer cela à MCW ; aucun code produit n'a été exécuté.
- Pour essayer un clic normal, un contrôleur complémentaire limité à **`Msvm_SyntheticMouse` de l'ID VM possédé** a été préparé et parsé. La nouvelle demande UAC a réellement retourné **« L'opération a été annulée par l'utilisateur »** via `Start-Process`. **Aucun accès souris élevé accordé ni clic exécuté**, aucun contournement ou relance automatique ; demander la décision utilisateur avant une nouvelle UAC.
- Le contrôleur initial reste la seule autorité de cycle de vie/clavier, avec l'échéance enregistrée ; ne pas créer une deuxième VM et ne pas détourner son jeton/console pour étendre ses opérations. Les identifiants synthétiques prévus pour le guest sont sauvegardés **chiffrés DPAPI CurrentUser**, hors Git et hors DVD, sans compte hôte créé.
- Reprise exacte : faire autoriser à nouveau le seul contrôle souris du guest, ou laisser l'utilisateur achever ce passage via la console **basique sans redirection**. Ensuite créer réellement le provisioning local, vérifier UUID firmware et politique PowerShell existante avant tout script, installer les outils autorisés puis ouvrir la vraie session standard. Les tests MSI/Cargo/portable, l'activation Windows et la qualification standard restent **non exécutés/non prouvés**.

### Reprise autorisée — souris virtuelle et compte local normal

- L'utilisateur a explicitement autorisé la **nouvelle** UAC souris après l'annulation précédente. Élévation complémentaire réellement réussie ; `Info` rapporte jeton admin actif, souris liée à **`260b15b3-0281-4520-aa6b-7ca208a01d26`** et résolution réelle **1024×768**. Aucun contrôle du pointeur/clavier de l'hôte.
- Clic normal **`Msvm_SyntheticMouse`** sur l'option officielle **Domain join instead** : **réussi**, écran de création de compte local affiché. Le problème d'activation au clavier ne nécessitait ni commande de bypass, ni compte Microsoft, ni modification de sécurité.
- Le compte synthétique de provisioning a été renseigné par les écrans Windows normaux, avec mot de passe protégé et trois questions de récupération obligatoires complétées avec des réponses synthétiques distinctes. Aucun secret affiché dans les captures/journaux ; les réponses sont récupérables depuis le secret DPAPI et les instructions privées de dérivation. Aucun compte hôte créé.
- Choix de confidentialité du guest observés : diagnostic **Required only**, saisie/écriture optionnelles **No**, offres personnalisées **No**. Le bouton normal Accept enregistre ces préférences du seul guest ; aucune protection Defender/UAC ou microphone/caméra hôte modifiée.
- Dernier écran : finalisation OOBE après ces choix. Attendre le bureau réel avant de revendiquer sa disponibilité ; vérifier ensuite le compte/UUID/politique PowerShell par le canal authentifié lié à la VM. La lecture directe du fichier d'état souris élevé a renvoyé `EPERM` depuis OMP, mais le canal contrôlé `Info/Click` fonctionne ; ne pas élargir des ACL pour lire ce fichier. Aucun test produit standard encore exécuté.

### Checkpoint authentifié — UUID réel et politique PowerShell autorisée

- Canal **PowerShell Direct authentifié lié à l'ID VM possédé** réellement fonctionnel avec le seul compte de provisioning synthétique. Identité locale/profil normaux lus dans le guest, jeton admin actif, Windows 11 Enterprise Evaluation build **26300**, volumes **C: OS, D: ISO Microsoft, E: MCW_QA_PAYLOAD**. Ce processus de remoting **n'est pas** une qualification graphique standard.
- **UUID firmware réel = `5A8D91CD-952C-47DA-9BB0-9F03479F500D`**, différent de l'ID Hyper-V `260b15b3-0281-4520-aa6b-7ca208a01d26`. Association établie par le canal authentifié sur cet ID, sans falsification. Passer l'UUID firmware comme **`-ExpectedGuestUuid`** aux deux scripts privés ; garder **l'ID Hyper-V** original pour toute gestion VM.
- Sécurité observée en lecture seule dans le guest : **Defender antivirus/realtime actifs, Tamper Protection active, UAC EnableLUA=1**. Politique PowerShell par défaut réellement **Restricted**, tous scopes indéfinis ; cela bloque les fichiers `.ps1`, pas un défaut du produit.
- L'utilisateur a donné un **troisième accord explicite**, limité aux deux comptes jetables du guest, pour **RemoteSigned / CurrentUser**. Application réelle seulement au compte de provisioning à ce checkpoint ; résultat effectif **RemoteSigned**, scopes **MachinePolicy/UserPolicy/Process/LocalMachine toujours Undefined**. Aucun Bypass/Unrestricted ni changement de politique hôte. Le compte standard reste à créer/connecter et configurer dans son propre contexte après autorisation déjà reçue.
- Preuves privées : `guest-readonly-initial-probe.json`, `guest-script-policy-consent.json`, `guest-setup-user-script-policy.json`. Dernier écran : mises à jour OOBE normales, **Step 3 of 3 Installing 94%** ; ne pas les assimiler à un bureau standard prêt ou lancer des installateurs en concurrence avec cette phase.

### Checkpoint — redémarrages de mise à jour et compte standard créé

- Les mises à jour normales OOBE ont redémarré **uniquement le guest**, interrompant une première requête de création de compte. Erreur PowerShell Direct conservée ; aucune répétition à l'aveugle. Lecture seule après un nouveau boot : **compte QA absent, aucun audit de création correspondant**. La création a été reprise seulement après cette preuve d'absence sur le nouvel OS démarré.
- Résultat réel : **`mcw_qa_c1c5de53` créé et activé**, membre du groupe Users `S-1-5-32-545`, **pas Administrators** ; SID du guest se termine par **`-1002`**. Preuve `created-guest-standard-account-post-reboot.json`. **Aucun logon interactif standard ni test MCW exécuté** à cette étape.
- Probe avant outils à **2026-10-25T15:36Z** : aucun reboot de servicing/Windows Update requis, installateur Windows Update **non busy**, aucun `msiexec` en cours, politique setup **RemoteSigned** conservée. OOBE affiche encore sa vérification normale de mises à jour ; la console est `defaultuser0`, pas le compte QA. Ne pas qualifier ce bureau ou un processus remoting comme session standard.
- Prochain travail : outils Machine autorisés via le compte de provisioning du seul guest, puis logon Windows réel QA, politique CurrentUser déjà autorisée appliquée dans **ce** contexte, outils User et snapshot propre avant le scénario unique. Source, profils et contrôles d'admission existants inchangés ; aucun logiciel installé sur l'hôte.

### Précondition enfin prouvée — véritable session graphique standard

- Logon QA effectué par le **vrai écran Windows Winlogon**. Une saisie groupée a échoué ; diagnostic SAM avec les mêmes identifiants **valide**, sans réinitialisation de mot de passe. Saisie virtuelle sérialisée ensuite **réussie**. Les questions/préférences normales du premier logon ont été traitées, sans token déplacé, `runas`, profil inventé ou ouverture par service.
- Bureau QA réellement affiché. Tâche locale de diagnostic **InteractiveToken / Limited**, attachée à ce logon déjà existant, utilisée uniquement pour lire le contexte et appliquer au **CurrentUser QA** le RemoteSigned déjà autorisé. L'export XML omet le RunLevel par défaut : l'ancien contrôle littéral l'avait arrêté avant démarrage. Vérification corrigée **sans modifier la tâche** : propriétés CIM effectivement `Interactive`/`Limited`, XML `InteractiveToken`, SID QA et action exacte contrôlés, aucune contradiction `HighestAvailable` admise.
- Reçu réel **`guest-actual-standard-interactive-context.json`**, tâche exit **0**, statut **`context-verified`** : session/token/LSA **2**, logon **2 Interactive**, **WinSta0 visible / Default / input Default**, Explorer du même SID/session **non élevé**, compte local privilege **1**, groupe Users seulement, token primaire **non élevé/non restreint**, profils connu/environnement/registre concordants et hive chargée. Ce résultat remplace le manque d'environnement interactif, **pas les scénarios MCW**.
- Runtime guest relu : **EnterpriseEval, 26H2, build 26300 UBR9711** après mises à jour, cohérent dans CIM/registre ; ne pas déduire la version runtime du fond/filigrane visuel plus ancien. Politique effective QA **RemoteSigned**. Toujours aucun produit MCW installé ni scénario MSI/Cargo/portable exécuté.
- Prochaine étape sauvegardée avant opération longue : exécuter le stade **Machine** préparé sur `E:` via le seul compte de provisioning et **ExpectedGuestUuid=5A8D91CD-952C-47DA-9BB0-9F03479F500D**, avec les deux consentements d'outils déjà reçus. Puis Rust dans le véritable contexte QA, snapshot propre et qualification unique à SHA indépendante. Ne pas substituer un processus de provisioning administratif au scénario standard.

### Arrêt sûr de diagnostic — provisioning long, résultat inconnu

- Appel réel du stade Machine dans le guest avec UUID/consentements autorisés. Le client a levé **`subprocess.TimeoutExpired` après 1800 secondes** ; cela ne prouve ni l'échec ni la réussite des installateurs distants. **Aucune relance**, aucun nettoyage, aucun succès de build/installation MCW revendiqué. Reçu privé `guest-machine-provision-client-timeout.json`.
- Le contrôleur initial traite les requêtes en série ; pendant ce long `GuestInvoke` il ne peut pas servir un autre diagnostic. Après perte du client, sa réponse peut échouer sur pipe fermé et déclencher son `finally`/arrêt du guest. **Ne pas conclure que le guest est éteint ou que les outils sont installés sans observation élevée réelle.** Une deadline contrôlée entre requêtes n'est pas une garantie d'interruption d'un appel bloqué.
- Inspecteur ponctuel **strictement en lecture seule**, lié à notre seul ID VM et à un script fixe de journaux/processus/outils, préparé. Sa demande UAC a réellement été **annulée** par Windows ; aucun nouveau droit ni diagnostic guest accordé. Échec conservé dans `readonly-inspector-launch-failure.json` ; ne pas redemander automatiquement.
- L'ancien canal souris répond encore mais refuse `Info` : **« Original one-VM authority is unavailable »**. Lecture des fichiers d'état élevés refusée `EPERM`. L'observation CIM/journaux non élevée n'a fourni **aucune ligne VM/événement exploitable**, ce qui n'est pas une preuve d'absence/arrêt. Aucun contournement de droits ou changement d'ACL.
- **Acquis à conserver : vrai contexte QA interactif entièrement vérifié**, profil/SID/LSA/WinSta0/Explorer corrects. **Inconnu : progression des outils Machine et état courant du guest. Non exécutés : Rust utilisateur, snapshot propre, compilation/qualification MCW MSI/Cargo/portable.** Pas de défaut produit démontré par ce timeout de préparation.
- Reprise obligatoire : nouvelle UAC explicitement acceptée pour inspecter **la VM existante `260b15b3-0281-4520-aa6b-7ca208a01d26`**, vérifier l'état et les journaux `%LOCALAPPDATA%/MCWPrerequisites-Machine-*` du setup avant toute mutation. Ne pas recréer de VM, rejouer le stade Machine, écraser un produit ou réinitialiser l'utilisateur. Si nécessaire rétablir un contrôleur borné à cette VM seulement après fin certaine de l'ancien ; exécuter les prochaines opérations longues comme tâches guest contrôlées et récupérer leurs reçus, sans bloquer l'unique canal d'observation.
- **NOT READY FOR RELEASE** demeure : le prérequis interactif est résolu, mais les scénarios réels du produit restent sans preuve. Aucune fusion `main`, tag, release ni déploiement.

### Reprise autorisée — résultat réel du provisioning récupéré

- Nouvelle UAC **explicitement autorisée et acceptée** après le checkpoint précédent. L'inventaire réel confirme l'ancien contrôleur **terminé**, son processus **absent** et notre VM existante **Off**. Un contrôleur de reprise borné au même ID a redémarré seulement cette VM ; aucune VM recréée, aucun changement hôte/commutateur/périphérique.
- Les journaux guest ont finalement levé l'incertitude : les **quatre installateurs Git/Python/.NET/Visual Studio Build Tools ont retourné 0**, et le reçu final Machine indique **`status=provisioned`, `rebootRequired=false`, `qualificationExecuted=false`**. Exécution réellement terminée à **16:46:57Z**, après environ **41 min 31 s** : le délai client de 30 minutes était inférieur à l'opération distante, pas une preuve d'échec du produit. **Aucun installateur rejoué.**
- Contrôle fonctionnel des prérequis après redémarrage : Git **2.56.0.windows.3**, Python **3.13.17**, SDK .NET **8.0.426** ; `vswhere` exige effectivement **MSVC x64 et Windows11SDK.26100**, et retourne VS Build Tools **17.14.37806.7**, `isComplete=true`, `isLaunchable=true`. **AudioEndpointBuilder/Audiosrv Running/Automatic**. Ces constats prouvent les outils, **pas une compilation MCW ou une qualification standard**.
- Preuves privées compactes : `machine-provision-verified-summary.json`, `verified-guest-machine-tools.json`, inventaire et démarrage du contrôleur de reprise. Les transcriptions brutes PowerShell Direct peuvent contenir un **jeton de transport éphémère dans l'en-tête Host Application** : ne jamais les commiter ni publier brutes ; exporter seulement des copies expurgées, sans aucune valeur de jeton.
- Canal de reprise : appels guest bornés, déconnexion client ne terminant plus automatiquement l'autorité ; opérations longues suivantes en **tâches locales guest**, observables séparément. Ne pas relancer le provisioning Machine réussi. Prochaine étape : Rust dans le **vrai logon QA InteractiveToken/Limited**, snapshot propre éteint, puis scénario produit unique au SHA source figé et examiné.
- Toujours **NOT READY FOR RELEASE** : aucun scénario MCW MSI/Cargo/portable exécuté dans ce guest à ce checkpoint. Droits/licences et RemoteSigned restent limités aux accords déjà enregistrés ; aucun `main`, tag, release ni déploiement.

### Bootstrap utilisateur — défaut du script privé isolé, pas du produit

- Après nouveau Winlogon normal, bureau QA réellement affiché ; première tâche Rust **InteractiveToken/Limited**, SID QA exact, Explorer en session **1**. Sortie réelle **1**, aucune création `.cargo`/`.rustup`, aucun téléchargement/installateur Rust, reçu de succès absent : **échec conservé**, jamais assimilé à une qualification.
- Inspection du script privé + reproduction de la seule boucle dans PowerShell : **`$home` entre en collision avec `$HOME` ReadOnly/AllScope**, erreur exacte **`VariableNotWritable`**. Le transcript s'arrêtait dans `finally` avant l'erreur extérieure ; ajout d'un lanceur privé capturant explicitement les erreurs et résultats, sans contourner la politique.
- Correction limitée au script de préparation **hors dépôt** : variable `$rustHome`. Nouveau script SHA-256 **`8a7f6fe8e9ec1e8c5da63d59d31dafec2a7ace5b5b10ebf8cb3adbb17344a217`**, source/lanceur réellement parsés dans le guest ; payload privé supplémentaire dans **ProgramData du guest seulement**, pas de partage hôte. Ancienne ISO, tâche et preuve d'échec conservées. Aucune modification du code MCW ni d'une protection.
- Nouvelle tâche `MCW-021-UserRustBootstrap-V2` enregistrée mais **non démarrée avant ce checkpoint** ; SID exact, propriétés CIM Interactive/Limited, XML InteractiveToken et action vérifiés, sans mot de passe/batch/S4U/HighestAvailable. Elle exécutera aussi un smoke jetable du véritable environnement VS sous le même compte, avant le snapshot et le scénario produit. Aucun nouveau défaut de MicCamWatch démontré.

### Chemin standard authentique — Explorer admis, tâche filtrée refusée

- V2 n'a installé aucun outil : le garde standard a refusé son jeton. Diagnostic **réellement exécuté sous QA**, reçu `actual-standard-context-after-bootstrap-refusal.json` : SID/Users/privilege=1 corrects, primary/default/non élevé, LSA Interactive=2, WinSta0/Default et Explorer corrects, mais **`hasRestrictions=true`**. **`restricted=false` ne suffit pas** : Microsoft documente TokenHasRestrictions comme l'historique d'un filtrage.
- Aucun affaiblissement du garde, jeton déplacé ou faux succès. Lancement de PowerShell par le **vrai Win+R d'Explorer dans le guest**, via clavier virtuel seulement. Reçu `ordinary-explorer-real-console-context.json` : **admission accepted**, même QA/session 1/logon 2/Explorer, **`hasRestrictions=false`, `restricted=false`**, token primaire/default non élevé, profil réel chargé. Ce chemin ordinaire résout le refus ; ne pas substituer la tâche planifiée filtrée au scénario.
- Les six dossiers personnels synthétiques sont réellement vides hors `desktop.ini` ; le raccourci Edge visible appartient au **Public Desktop**, sans suppression. Aucune installation MCW ni mutation de sécurité/périphérique. Prochaine opération : lancer le V2 corrigé **normalement depuis Explorer**, puis conserver reçus Rust/smoke VS et snapshot propre éteint avant l'unique qualification produit.

### Cause démontrée — lecture hors tampon du BOOLEAN TokenHasRestrictions

- **Correction du diagnostic précédent :** un `hasRestrictions=true` produit par l'ancien lecteur ne prouvait pas un filtrage réel de la tâche ou d'Explorer. La mesure native indépendante renvoie **un octet** pour la classe **21**, alors que `TokenInt` allouait ce seul octet puis utilisait **`Marshal.ReadInt32`**. Trois octets hors tampon pouvaient rendre un jeton standard faussement filtré. L'erreur native **`Unexpected DWORD length 1`** est conservée dans `raw-native-token-information-error.json`.
- Avant/après réellement exécuté dans le même guest, même QA/Winlogon/WinSta0/Explorer, contre un oracle natif à tampon d'octets initialisé. Le booléen natif reste **false**, aucune sécurité ni identité modifiée :

  | Lecteur | Largeur native | Décodages incorrects | Admissions standard |
  | --- | --- | --- | --- |
  | Original, SHA `1ee6929c…` | 1 octet | 5 / 8 | 3 / 8 |
  | Corrigé, SHA `19f3ca1d…` | 1 octet | 0 / 8 | 8 / 8 |

- **`installer/tests/windows-fixture-context.ps1`** : lecture scalaire dans un entier **initialisé et passé par ref**, sans allocation native ni lecture hors tampon, acceptant seulement les tailles scalaires attendues et le BOOLEAN d'un octet. **Aucune condition de `Assert-McwStandardUserContext` supprimée ou assouplie** ; les vraies restrictions restent refusées.
- **`installer/tests/windows-context-smoke.ps1`** : oracle indépendant à struct de quatre octets, comparaison du drapeau observé avec l'API native avant admission ; régression de frontière ABI, pas un mock ni une assertion de texte source.
- Smoke hôte **strictement en lecture seule** réellement passé : quatre scripts parsés, quatre types C# compilés, largeur native un octet, contexte administratif et hôte physique correctement refusés ; **`qualificationStatus=not_executed`, `productActionsInvoked=false`**. Aucun produit, périphérique, profil ou politique hôte modifié.
- Preuves privées : `native-token-original-helper-before-proof.json`, `native-token-fixed-helper-after-proof.json`, `native-token-abi-regression-verified-summary.json`. Nouvelle préparation privée V3 liée au helper exact **`19f3ca1dbc0b445e42a1330d10f41602468288766e87b96bac68ccd4eea986bb`**, source/lanceur parsés et conservés, **pas encore exécutés avant ce checkpoint**. Anciennes preuves/ISO/fichiers inchangés.
- Prochain travail : sauvegarder cette correction du **harness**, contrôler le smoke CI Windows ciblé, puis Rust depuis le bureau standard ordinaire, snapshot propre éteint et scénario complet au SHA figé. Pas de changement du cœur Rust/GUI/installateur produit, pas de nouvelle qualification Unix requise pour cette lecture C# Windows seule. **NOT READY FOR RELEASE** tant que les reçus produit standard ne sont pas complets.

### Prérequis standard et CI vérifiés — redémarrage guest avant snapshot

- Correctif sauvegardé/poussé **`5795ed4bab7952a571237c6f99df54241c698af6`**. Smoke CI ciblé **[40225379744](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40225379744)** réellement réussi sur ce SHA ; étapes et artefact **`windows-context-smoke` / 12610233987** téléchargés et lus. Reçu : quatre scripts parsés, quatre types compilés, référence native **un octet / false**, contextes administratifs/modes mixtes refusés, **aucune qualification produit**. Les jobs Unix/Windows produit/standard sont volontairement **non exécutés** pour ce dispatch `windows-context`.
- **V3 exécuté normalement depuis Explorer**, pas via remoting ou faux logon : stade User **`status=provisioned`, rustup exit 0**, profil QA réel ; aucune réinstallation Machine. Le canal stable a annoncé **1.99.1**, mais stdout final dit **`timeout reading rustc version`** : ne pas prétendre que `rustc --version` ou un build ont été vérifiés.
- Smoke du **véritable `VsDevCmd.bat` x64** et de son child PowerShell terminé **`prerequisites-and-environment-ready`**. Reçu réel : QA/Users/privilege 1, token primaire/default non élevé et **sans restrictions**, logon 2 / session 1 / WinSta0/Default, Explorer QA et profil chargé ; chemins **cl.exe MSVC14.44.35207** et **rc.exe SDK10.0.26100.0** résolus. Ce smoke prouve l'environnement/identité, **pas une compilation**.
- Lecture avant snapshot : aucun processus outil/installateur/MCW actif hors console de diagnostic, **aucun reçu immuable de tentative produit**. Defender antivirus/realtime/Tamper Protection toujours actifs, UAC **EnableLUA=1** ; VM toujours Gen2/4vCPU/RAM6GiB/dynamique8GiB/TPM/SecureBoot, un seul VHDX privé et aucun pass-through.
- **Nouveau rebootPending=true** observé dans le guest après les mises à jour normales. Ne pas prendre un checkpoint « propre » avec ce prérequis en attente. Prochaine étape : redémarrage normal de **ce guest uniquement**, vérifier fin de servicing et versions réelles Rust/Cargo sans réinstallation, puis arrêt/checkpoint propre et nouveau Winlogon QA avant scénario unique.
- Entrée du scénario **préparée, non lancée**, source figée indépendante **5795ed4…** (même code que le smoke CI), wrapper privé SHA **`7606c62e434223613ca655b39055410f927f1d7c5499086d170581e70c93efd3`**, erreurs précoces capturées et reçus CreateNew anti-rejeu. Les commits documentaires ultérieurs n'en changent pas le code testé. Aucun scénario MSI/Cargo/portable exécuté à ce point ; verdict inchangé.

### Baseline propre réellement sauvegardée — checkpoint avant scénario produit

- Redémarrage Windows normal du **seul guest** demandé à 19:34:52Z ; boot réellement relu **19:37:27Z**, servicing/Windows Update **plus aucun reboot en attente**, installateur Update non busy. Un refus remoting transitoire a été conservé ; lecture Hyper-V Running puis nouvelle lecture guest ont établi l'issue réelle, sans démarrer/forcer une VM supposée éteinte.
- Premier probe privé : stdout valide `rustc 1.99.1`, mais **ExitCode null** renvoyé par le wrapper PowerShell `Start-Process` sans `-Wait` ; jamais converti en 0 ni attribué à Rust. Probe privé corrigé avec **`System.Diagnostics.Process`**, lectures asynchrones et délai borné, sans réinstallation ni changement du produit.
- Reçu **`verified-real-standard-rust-cargo-tools.json`** : exécutions réelles **rustc 1.99.1 (58cf8f9c3)** et **cargo 1.99.1 (5f94df478)**, **deux ExitCode 0**, stderr vides, garde QA complet admis (Winlogon/LSA 2, WinSta0/Default, profil chargé, Users seulement, token primaire/default sans restriction, Explorer non élevé).
- Admission fraîche avant snapshot à **19:50:33Z** : aucun outil/produit actif, aucun reçu immuable produit, services AudioEndpointBuilder/Audiosrv Running/Automatic, aucun reboot pending. Arrêt réel de la VM, **état Off relu**, puis premier checkpoint **`MCW-Clean-Standard-User`**, ID **`159b57e0-155e-4914-98ea-e8c686a786eb`**, lié au seul ID Hyper-V possédé.
- Ce checkpoint contient l'OS/licences/outils/profil normaux et les preuves privées, **aucun MicCamWatch installé ni tentative MSI/Cargo/portable**. Ne pas supprimer les preuves des probes antérieurs ni rejouer une tentative produit sur son profil après échec. Toute récupération/restauration ultérieure doit rester confinée à cette VM, avec autorité Hyper-V encore effective.
- Reprise immédiate : redémarrer cette même VM, Winlogon QA normal, puis **entrée console unique source `5795ed4bab7952a571237c6f99df54241c698af6`** déjà préparée ; conserver tous reçus et états même en cas d'échec. La branche a seulement avancé par documentation depuis ce source de qualification. Verdict toujours **NOT READY FOR RELEASE** avant les preuves produit.

## 2026-10-25 — Pause passive demandée, reprise le 26 à 00 h 13 Europe/Paris

### État exact au départ en pause

- Demande reçue **20:05:29Z / 21:05:29 Paris** ; arrêt des nouvelles opérations au plus tard **20:20:29Z**, soit les quinze minutes autorisées. Reprise demandée **2026-10-26 00:13:00 Europe/Paris = 2026-10-25 23:13:00Z** (CET, après le changement d'heure).
- Tentative unique source **`5795ed4bab7952a571237c6f99df54241c698af6`** réellement lancée depuis le Winlogon/Explorer QA : entrée **20:03:32Z**, sortie **1 à 20:04:03Z**. Clone/admission réels, puis refus du **lanceur privé** : **`A real provisioned Python interpreter is required; the Microsoft Store execution alias is not sufficient.`**
- **L'orchestrateur n'a pas démarré**, son dossier de preuves est absent, aucun build MCW, doctor, package MSI, installation Cargo/portable ou scénario produit n'a été exécuté dans cette tentative. Aucun processus MCW/Cargo/Rust/Python encore actif à la sauvegarde. **Ce n'est ni un PASS, ni une défaillance démontrée du produit.**
- Python réel **`C:\Program Files\Python313\python.exe`**, version **3.13.17**, signature **Valid / Python Software Foundation** ; Machine PATH contient ses répertoires. La sélection dans le **processus QA** a pourtant trouvé l'alias Store : sa cause précise reste à établir, sans présumer un problème d'ACL/profil. Le User PATH brut contient bien **`%USERPROFILE%`**, pas le profil setup : lire les REG_EXPAND_SZ avec **DoNotExpandEnvironmentNames**, sinon le lecteur admin les développe trompeusement dans son propre profil.
- Preuves conservées dans le guest **et** le répertoire privé hôte `mcw-021-eval-20261025-ngbjedk9` : `actual-full-qualification-first-authoritative-observation.json`, `actual-python-alias-prebuild-failure-diagnosis.json`, `retained-actual-failed-admission-before-passive-pause.json`, transcript expurgé, reçus immuables d'entrée/lancement et contexte d'admission. Ne jamais les commiter en bloc ni publier les anciens transcripts PowerShell Direct bruts.
- VM **`MCW-021-QA-c1c5de53` / `260b15b3-0281-4520-aa6b-7ca208a01d26` arrêtée**, état **Off réellement relu**, disque/échec conservés sans désinstallation/nettoyage. Checkpoint propre **`159b57e0-155e-4914-98ea-e8c686a786eb` / MCW-Clean-Standard-User** intact. Aucune autre VM/worktree/installation hôte ni protection micro/caméra modifiée.
- Le contrôleur de reprise élevé reste borné à cette VM, expiration annoncée **2026-10-26 00:15Z / 01:15 Paris**. Il sait Start/Shutdown/Checkpoint, **pas Restore**. Ne pas inventer cette capacité ni contourner une nouvelle UAC nécessaire à la restauration ; aucune restauration n'a été tentée. Le réveil prévu précède cette expiration, mais ne prolonge pas les droits.

### Attente vérifiée et consignes au réveil

- Documentation OMP effectivement lue : **bash `timeout:0` désactive toute deadline**, le **PTY foreground** évite l'auto-background non-PTY ; **`wait` possède au contraire un plafond de trente minutes**, donc ne pas le rappeler en boucle. Probe réel du même timer/PTY terminé dans cette session, **`timeoutDisabled=true`**, sans fallback PTY indisponible.
- Un seul script local privé **`passive-wait-until.py`** dort jusqu'au UTC cible : aucun réseau, modèle, sous-agent, tâche planifiée, relancement OMP ou secret. Unique commande prévue, en **bash foreground `pty:true`, `async:false`, `timeout:0`** :

  ```text
  C:/Users/stcui/AppData/Local/Programs/Python/Python312/python.exe C:/Users/stcui/AppData/Local/Temp/mcw-021-eval-20261025-ngbjedk9/passive-wait-until.py --until-utc 2026-10-25T23:13:00+00:00 --state C:/Users/stcui/AppData/Local/Temp/mcw-021-eval-20261025-ngbjedk9/passive-wait-state-20261026-0013.json
  ```

- La fin de **ce même appel outil** rend la main à la session OMP ; pas de session terminée/recréée ni de polling modèle pendant le sommeil. Condition : hôte/OMP restent actifs ; fermeture, sommeil du PC ou arrêt par le **bloqueur à 15 %** peuvent interrompre/retarder le réveil. **Le bloqueur reste prioritaire** : aucun mécanisme de contournement ou relancement. Une heure atteinte **ne prouve pas** un quota renouvelé ; pas d'API fiable vérifiée, pas d'appel de vérification pendant la pause. `WATCHDOG.yml` du dépôt ne contient qu'un advisor, pas une preuve d'état du bloqueur externe.
- Au réveil : **relire Git et cette section**, préserver les worktrees/cinq exclusions historiques, réutiliser le kernel/autorité encore valides sans réexécuter les setups réussis. Inspecter les reçus privés, confirmer VM Off/point propre et droits restants, puis diagnostiquer la résolution Python dans un **vrai processus QA** ; vérifier l'interpréteur installé avant tout réglage de PATH limité au processus. Ne pas changer le profil, les ACL ou désactiver globalement des alias pour masquer le refus.
- **Ne pas rejouer la tentative sur ce profil** : conserver l'échec et restaurer/reprovisionner uniquement le guest autorisé à partir du point propre, avec nouvelle UAC explicite si l'opération n'est pas disponible. Ensuite seulement reprendre l'entrée de qualification complète au SHA examiné, sans refaire les CI/provisionings réussis. Si l'autorité ou le bloqueur empêche l'action, sauvegarder et demander seulement la permission manquante.
- **NOT READY FOR RELEASE** : gate produit standard non exécuté ; aucune fusion `main`, aucun tag/release, aucun déploiement. Prochaine preuve attendue : interpréteur Python QA correctement résolu, puis reçus complets orchestrateur/native/même MSI `standardUserQualified` et `cleanupSafe`, jamais un succès déduit du seul timer.

## 2026-10-26 — Réveil passif réussi et correction de découverte Python

- Attente réellement terminée dans **le même appel PTY/session**, de **20:20:00.253Z à 23:13:00.000Z**, soit l'heure Paris demandée. Aucun polling modèle, sous-agent, tâche Windows ou relancement OMP ; advisor effectivement désactivé. Relecture Git : branche candidate propre/synchronisée à **`3fd037146b18da369224a9ab23842acee98ec04d`**, `origin/main` toujours **dc40e95** ; la vieille branche locale `main` n'a pas été réécrite. La minuterie ne prétend pas mesurer le quota du fournisseur.
- Autorité existante encore active au réveil, VM initialement Off ; démarrage de **la seule VM possédée**, Winlogon QA normal et diagnostic **strictement en lecture seule**, sans rejouer l'entrée immuable échouée.
- **Cause précise enfin mesurée : aucun PATH cassé.** `Get-Command python.exe -CommandType Application` renvoie **deux ApplicationInfo**, Python PSF en premier et Store ensuite. `.Source` devient **System.Object[]** ; `.StartsWith(...)` donne **`[false,true]`**, dont la conversion en booléen vaut **true**. Le faux refus venait de cette liste traitée comme un chemin scalaire, pas d'un Python absent ou prioritairement remplacé par Store. Python réel exécuté sous QA : **3.13.17, ExitCode 0**.
- Correction **`Select-Object -First 1`** selon la précédence normale, sans rechercher artificiellement un résultat ultérieur : **windows-standard-user.ps1** (Git/Cargo/.NET/Python), **windows-tray-profiles-native.ps1** (Cargo), **.github/workflows/native-uninstall.yml** (Python) et lanceur privé hors dépôt. Le contrôle anti-alias Store reste identique ; le contrôle de présence de tout produit existant continue volontairement à examiner **tous** les résultats.
- Smoke réel QA après correction : quatre sources **System.String**, Python détecté deux fois mais sélectionné **`C:\Program Files\Python313\python.exe`**, exécution **ExitCode 0**, stderr vide. **Aucun PATH/profil/ACL modifié, alias global inchangé, aucun produit exécuté.** Preuves `actual-qa-python-resolution-proof-after-pause.json` et `actual-selected-native-tools-proof.json`.
- Validation ciblée : smoke contexte/parsing/C# hôte **PASS**, toujours **qualificationStatus=not_executed / productActionsInvoked=false**, refus admin/hôte physique conservés. Parser PowerShell réel : orchestrateur, profil-native, lanceur privé et **bloc run YAML réellement modifié** sans erreur ; YAML chargé avec le parseur existant, aucune installation de dépendance hôte.
- **Huit fichiers de l'échec initial et des probes exportés byte-for-byte et SHA-256 vérifiés**, avant toute restauration, dans `retained-failed-attempt-5795ed4-before-any-restore.json` privé, SHA **`f4cadcbf48e905470e231079b37513006fe490944134b8c662fbe88920e02fe9`**. Aucune restauration effectuée ; échec original/point propre conservés.
- VM à nouveau **arrêtée, Off relu**. Reprise produit encore conditionnée à une restauration propre du checkpoint **159b57e0…**, pas à une suppression de ses reçus ou un rejeu du profil. Le contrôleur actuel ne possède pas l'opération Restore : demander une **nouvelle autorisation/UAC bornée à cette VM et ce checkpoint**, sans chevaucher deux propriétaires, puis reprendre sur le nouveau SHA sauvegardé. Aucun changement Rust/GUI/installateur produit, aucune fusion `main`, tag, release ou site ; **NOT READY FOR RELEASE** jusqu'aux reçus complets standard.

### Nouvelle UAC acceptée — restauration propre réellement vérifiée

- L'utilisateur a explicitement choisi **« Autoriser l’UAC de restauration »**, puis Windows a effectivement lancé le nouveau processus élevé. L'ancien canal Close était déjà indisponible ; **PID34644 réellement absent**, vérifié avant tout nouveau lancement, pas deux propriétaires concurrents ni réussite Close inventée.
- Nouveau contrôleur privé borné à **la même VM**, **PID16232**, jeton admin actif et commandes Hyper-V réellement fonctionnelles, expiration **2026-10-26 08:22:20Z / 09:22 Paris**. Une seule opération RestoreClean, point/VM fixés, VM Off obligatoire, archive d'échec au SHA attendu et intention CreateNew anti-rejeu ; aucun compte/logiciel/protection hôte changé.
- **Restauration réussie à 06:24:09Z** du seul checkpoint **159b57e0-155e-4914-98ea-e8c686a786eb**, état final **Off réellement relu**. L'ancien état guest a été remplacé par le point propre comme autorisé ; ses huit preuves restent dans l'archive privée byte-vérifiée hôte, pas prétendument dans le profil restauré.
- Redémarrage normal de cette VM, profil restauré **sans reçus immuables de tentative**, aucun processus produit, AudioEndpointBuilder/Audiosrv Running/Automatic et **aucun reboot pending**, tous réellement relus. Aucun outil réinstallé, aucun reçu effacé manuellement.
- Source figée **`30051ce70e904912d988d17ad8e72cf4d19c6568`** ; smoke CI **[40245298208](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40245298208)** passé sur ce SHA, artefact **12618646305** téléchargé et lu (quatre scripts/types, drapeau natif un octet, refus admin/modes mixtes corrects, **aucune qualification produit**).
- Nouveaux payloads privés parsés et vérifiés dans le guest : wrapper scalaire SHA **`40c957baf7af312ceeeee6ef09fc27905bed443a25772f260830b944a33286be`**, entrée source figée SHA **`2e3a4f16523711b11c438a328c94af3453ab79d06cbe81c1b170cf4368e7b643`**. L'ancienne ISO/lanceur et l'archive d'échec ne sont pas écrasés. Winlogon QA normal accepté ; prochaine étape : depuis son Explorer, **une seule nouvelle tentative** avec ces payloads, puis lire tous les reçus. **NOT READY FOR RELEASE** avant ce résultat.

### Tentative source 30051ce — compilation réelle en cours

- Nouvelle entrée unique depuis **le vrai Explorer QA** à **06:31:55Z**, après restauration propre, source **30051ce…**, wrapper **40c957ba…**. Répertoire réel `MCWQualification-20261026-063157781-0c7b3e9e25fe4837af7cb448416eaba9`. Le faux refus Python n'apparaît plus.
- Orchestrateur effectivement démarré **06:32:55Z**, kind **local-disposable-vm**, contexte standard réel ; Git révision/checkout propre ont chacun retourné **0**. Commande réelle **`cargo build --release --locked --features windows-tray --bin mcw --bin mcw-tray`** lancée **06:33:17Z** ; à **06:37:57Z**, cargo/rustc QA session 1 actifs, dépendances téléchargées et compilation observée. **ExitCode null = commande encore en cours**, pas un PASS ou un échec.
- Diagnostic privé de logs ajusté pour ouvrir un journal actif avec **FileShare.ReadWrite/Delete** et signaler explicitement toute indisponibilité ; le premier refus de lecture d'un fichier détenu par le compilateur n'était **pas** un échec de build. Aucun changement du produit, arrêt de processus ou relance du scénario.
- Reçu `actual-scalar-fixed-full-scenario-first-authoritative-progress.json`. **Pas encore de doctor/paquet/MSI/Cargo-install/native/application final** : attendre les reçus complets, puis contrôler l'inventaire zéro périphérique et les résultats standard/cleanupSafe. Aucune qualification GUI ou publication revendiquée.

### Compilation réelle réussie — faux refus du JSON Windows PowerShell 5

- Sur **`30051ce70e904912d988d17ad8e72cf4d19c6568`**, la commande réelle de compilation commencée à **06:33:17Z** s'est terminée à **07:14:59Z**, **ExitCode 0**, après **41 min 40 s**. Les deux exécutables ont réellement été produits dans le profil standard ; aucune compilation rejouée pour confirmer ce résultat.
- Le vrai **`mcw doctor --lang en --json --no-color` a retourné 0** et rapporté séparément **`microphone_devices` / warning / `0 capture device(s) detected`** et **`camera_devices` / warning / `0 camera device(s) detected`**. Le gate a néanmoins arrêté l'orchestrateur à **07:15:08Z**, avant WiX/MSI/Cargo-install et tous les scénarios natifs. **Ce n'est pas une preuve de périphériques présents ni d'un défaut de MicCamWatch.**
- Cause reproduite sur **Windows PowerShell 5.1** avec le JSON réellement conservé : **`@(Get-Content … | ConvertFrom-Json)` contient un objet tableau**, au lieu des dix contrôles directement accessibles. Les propriétés deviennent des listes et le prédicat strict refuse à tort les deux observations. Correction dans **`installer/tests/windows-standard-user.ps1`** : affecter directement le résultat de `ConvertFrom-Json`. **Nombre de correspondances, statuts attendus et inventaires positifs zéro restent inchangés.**
- Vérification réelle du consommateur corrigé, isolé par l'AST PowerShell du fichier source, avec la sortie `doctor` originale byte-vérifiée : **PASS** pour zéro positif ; **refus conservés dans six cas** — contrôle manquant, doublon caméra, périphérique micro présent, erreur d'énumération caméra, inventaire inconnu et statut inattendu. **PowerShell 5.1.26100.9739** ; aucun binaire produit, MSI ou contrôle de périphérique exécuté par ce smoke. Le smoke existant contexte/parsing/C# est également **PASS**, toujours **qualification non exécutée**.
- **18 reçus/journaux de cette tentative exportés byte-for-byte et SHA-256 vérifiés** dans l'archive privée hôte **`retained-30051ce-build-doctor-and-initial-failed-orchestrator.json`**. Le reçu initial reste **failed**, pas réécrit en succès. Empreintes des vrais binaires conservés dans le disque guest : **mcw.exe `76c63f49c958520ba736912c2f10fd30527584f6baa0911a324dc77e3964452c`**, **mcw-tray.exe `3f19cfecf0bd9b66ee769d7853689553eadfd9735ac0bb13a20a685cab4feffa`**.
- Après constat d'absence de processus produit/compilation actif, **seule la VM temporaire a été arrêtée ; Off réellement relu**. Ses binaires, caches et preuves sont conservés ; aucun produit n'a encore été installé ou désinstallé dans cette tentative. Hôte, autres VM/worktrees, données, protections, site et branches protégées inchangés.
- **Prochain point de reprise :** l'orchestrateur interdit de rejouer un répertoire échoué et le contrôleur a déjà consommé son unique restauration autorisée. **Ne supprimer aucun reçu ou verrou de restauration.** Soit obtenir une nouvelle UAC de restauration et refaire une tentative complète propre sur le SHA corrigé, soit obtenir une décision explicite autorisant une continuation tracée des seules étapes restantes à partir de ces preuves de compilation/doctor, sans masquer l'échec initial ni déroger aux contrôles standard/hardware/MSI/native. Aucune continuation ou nouvelle restauration n'a été improvisée.
- **NOT READY FOR RELEASE** : compilation et inventaire ne suffisent pas ; les reçus natifs complets et du même MSI **passed / standardUserQualified / requireStandardUser / cleanupSafe** manquent toujours. Aucun changement Rust/GUI/installateur, fusion `main`, tag, release ou déploiement.

### Décision utilisateur — continuation tracée explicitement autorisée

- Après présentation des deux protocoles, l'utilisateur a choisi **« Poursuite tracée sans recompiler »**. Cette autorisation vise **les seules étapes jamais exécutées**, pas le rejeu ou la transformation en succès du reçu initial. Le protocole par défaut de nouvelle tentative propre reste inchangé.
- **Code compilé inchangé : `30051ce70e904912d988d17ad8e72cf4d19c6568`** ; correction du harness sauvegardée/poussée **`9baff90cef7d2528d86d1562eca022211663a636`**. Le futur reçu doit distinguer ces deux provenances, pas prétendre que les exécutables ont été compilés sur 9baff90.
- Payload privé unique **`authorized-staged-continuation.ps1`**, SHA **`4a31c9db54aa4268be49eb6efb77001ded74eb82116d97a7c84e6cf19c318f4a`**, et entrée Explorer **`d6560c2075de2cef3d1f01dbfd22162c5783b58fc0a711f450dafbb483e4a64a`** : identité/UUID/contextes réels, dix-huit empreintes des preuves initiales et les deux binaires figés vérifiés ; état produit préexistant refusé, marqueur CreateNew et **nouveau répertoire de preuves** `mcw-standard-user-continuation-proof`. Aucun fichier suivi guest ni reçu initial modifié.
- Parser guest **0 erreur** ; extraction AST indépendante **PASS** : **23 instructions originales**, depuis WiX restore jusqu'aux exigences finales natives/MSI, avec commandes réelles **wix-restore, cargo-metadata, wix-build, cargo-install, native-uninstall** et **aucun cargo-build**. L'installation Cargo reste une véritable commande Cargo offline, pas une copie déguisée.
- Avant toute mutation, la continuation doit revérifier le checkout compilé propre, l'absence d'installation/données étrangères, le contexte standard interactif et un **nouveau doctor positif zéro après le redémarrage**. Elle ne peut réussir qu'avec les reçus complets natifs et du **même MSI** `passed / standardUserQualified / requireStandardUser / cleanupSafe`, puis confirmation que les dix-huit fichiers initiaux sont intacts.
- **Pas encore de résultat natif revendiqué.** VM redémarrée, Winlogon QA normal accepté ; entrée de continuation à lancer depuis **son vrai Explorer**, jamais par remoting QA. Autorité Hyper-V toujours finie à **08:22:20Z / 09:22 Paris**, sans prolongation cachée ; nouvelle UAC si elle devient indispensable. Le précédent échec reste conservé, verdict **NOT READY FOR RELEASE** avant les nouvelles preuves.

### Continuation effectivement entrée — MSI produit, vraie installation Cargo réussie

- Entrée **Explorer QA ordinaire à 08:03:07Z**, nouveau reçu à **08:03:41Z**, kind **explicitly-approved-local-staged-continuation**, source compilée **30051ce…**, fix **9baff90…**, **cargoBuildReplayed=false**. Les dix-huit empreintes originales et les deux binaires ont passé les gardes ; l'ancienne tentative n'a pas été rejouée.
- Git révision/checkout propre **ExitCode 0**, puis nouveau doctor **ExitCode 0** à **08:03:45Z**, toujours **zéro capture et zéro caméra positivement observés**. Admission réelle et absence de produit/données préexistants ont réussi sans changement de droits ou d'environnement de profil.
- **WiX 5.0.2 restore ExitCode 0** à **08:03:56Z**, métadonnées Cargo **0**, **MSI authentique version 0.21.0 construit, ExitCode 0 à 08:04:21Z**, depuis les deux binaires conservés. Ce paquet est une preuve locale de test, pas une release publiée.
- **Vraie commande `cargo install --locked --offline --path <source> --root <fixture>/mcw-cargo-uninstall --features windows-tray` réussie**, **08:04:21Z → 08:05:06Z, ExitCode 0** ; Cargo a réellement installé **mcw.exe et mcw-tray.exe**. Pas de copie substituée à une installation Cargo, ni de relance de l'étape cargo-build antérieure.
- Nouveau contexte standard contrôlé immédiatement avant le consommateur ; **`native_uninstall.py` effectivement lancé à 08:05:06Z**, avec MSI/Cargo/source/binaire/proof réels et flags locaux, **ExitCode encore null au reçu 08:05:07Z**. **Ce n'est pas encore un PASS natif ou GUI.** Répertoire des nouvelles preuves : **`mcw-standard-user-continuation-proof`** ; ancien orchestrateur toujours **failed**, nouveaux résultats natifs/même MSI en attente.

### Échec natif conservé — chemin temporaire trop imbriqué, aucun produit encore installé

- Reçu de continuation **failed à 08:05:28Z**, natif **ExitCode 1**. Le natif a positivement validé **vrai contexte interactif standard**, **kind local-disposable-vm**, **zéro matériel de capture** et **jeton ordinaire réel**. L'ancien orchestrateur et ses dix-huit fichiers restent inchangés ; le nouvel échec n'est pas un PASS.
- Cause exacte du traceback : **`FileNotFoundError [WinError 206]` dans `native_uninstall.py:227`, `directory.mkdir()`**, au tout premier cas **« successful removal without frontend »**, **avant copie ou exécution du produit**. Le répertoire demandé compte **248 caractères**. L'empilement **fixture → proof → temporary-state → sandbox → libellé → portable installation** épuise la marge Win32 ; ce n'est pas un nouveau refus du tray ou du MSI.
- Correction ciblée de **windows-standard-user.ps1** : placer **TEMP/TMP dans `<racine admise>/mcw-tmp`**, toujours sous la même racine réellement possédée, sans modifier USERPROFILE/APPDATA, droits, registre ou politique des chemins longs. **windows-standard-user.yml** conserve explicitement cette nouvelle arborescence dans les artefacts ; guide mis à jour. Aucun changement au code natif du produit ni aux scénarios/assertions de sécurité.
- **33 fichiers supplémentaires exportés byte-for-byte et SHA-256 vérifiés** dans l'archive privée **`retained-native-winerror206-continuation-failure.json`** ; MSI réel SHA **`bbc2903e4de5eb4cde34fc827dfcf0f5472063016d71091d2c2ffcc626751b58`**. MSI et véritable installation Cargo restent disponibles dans le disque guest ; aucun replay du build ou de ces installations nécessaire.
- **VM arrêtée proprement, Off réellement relu**, sans processus produit/compilateur actif avant arrêt. Qualification native/MSI encore incomplète ; **NOT READY FOR RELEASE**. Le prochain essai doit avoir de nouveaux répertoires de preuves/temporaires, conserver les deux échecs, réutiliser uniquement les artefacts réels vérifiés et garder tous les contrôles natifs standard/zero-hardware/MSI/cleanupSafe.
- Nouvelle demande utilisateur de nettoyage : **preuves à transférer dans un emplacement durable**, ISO officielle à conserver sans copie, puis suppression uniquement des temporaires propres à la mission. **Ne supprimer la VM/disques possédés qu'après fin des tests, preuves durables et absence de reprise nécessaire** ; avec l'échec actuel, conserver la VM. Autres VM/worktrees, fichiers personnels, installations MicCamWatch et caches globaux partagés exclus ; mesurer l'espace avant/après tout nettoyage effectif. Aucun nettoyage destructif commencé.

### Correction des chemins vérifiée et preuves mises hors de Temp

- Smoke réellement exécuté sous **Windows PowerShell 5.1.26100.9739**, **ExitCode 0**, en réutilisant par AST **l'affectation `$fixtureTemp` du fichier corrigé** : racine hôte synthétique de **107 caractères**, comme la racine guest ; même hiérarchie native que l'échec. Le répertoire demandé passe de **248 à 203 caractères**, création réelle et écriture/relecture d'un fichier **218 caractères réussies**, toujours sous la racine possédée. **Aucune politique de chemins longs ou action produit modifiée/exécutée.** YAML du workflow également parsé sans erreur. Ce smoke n'est pas une qualification native complète.
- Registre des commandes du natif échoué effectivement lu : **admission PowerShell 0**, puis **doctor d'observation 0**, avant l'échec de création du premier répertoire portable. **Aucune commande d'installation/désinstallation, activation de protection ou lancement tray de ce cas n'a été exécutée.**
- Quatre preuves/archives utiles désormais sauvegardées dans **`%LOCALAPPDATA%\MicCamWatchQualification\0.21.0-20261026-c1c5de53`**, hors des fichiers temporaires : les deux archives byte-vérifiées des échecs/build/doctor, le smoke des six refus d'inventaire et celui des chemins. **Relecture SHA-256 des quatre fichiers réussie**, manifeste **`checkpoint-before-native-path-retry.json`** avec provenance et état partiel. Identifiants de récupération, mots de passe et transcriptions PowerShell Direct brutes exclus ; aucun jeton `-Token` brut dans les exports.
- Mesure réelle **C: à 08:26:59Z, 76 339 597 312 octets libres / 71,097 Gio**, avant toute suppression de cette mission. L'observation utilisateur antérieure **69,6 Go** est conservée mais n'est pas substituée à ce relevé. **Aucun espace récupéré par nettoyage n'est revendiqué**, puisqu'aucune suppression n'a commencé.
- La VM était **Off relu après l'échec** ; ses artefacts MSI/Cargo réels et tous les états de diagnostic sont conservés. L'autorité finie **08:22:20Z** est dépassée : **ne pas la prolonger ni relancer la VM sans nouvelle UAC explicite**, pas de workaround par tâche/service/compte. Nouvelle entrée native à prévoir dans un répertoire neuf avec le TEMP aplati, réutilisation des artefacts réellement installés/produits, reçus précédents intacts ; **NOT READY FOR RELEASE** reste applicable.

### Nouvelle UAC vérifiée — entrée native seule préparée, sans refaire les étapes réussies

- L'utilisateur a explicitement choisi **« Autoriser la nouvelle UAC VM »**, puis l'élévation Windows a réellement lancé **PID26092**. **Ancien PID16232 absent** avant lancement ; nouvelle Inventory confirme **adminTokenActive=true**, commandes Hyper-V opérationnelles, ancien état **closed** et aucun ancien propriétaire actif.
- Autorité privée **`native-controller.ps1`**, bornée à la même VM **260b15b3…**, expiration **2026-10-26 10:34:48Z / 11:34 Paris**. Capacités volontairement réduites : Inventory/Configuration/Screen/Start/Shutdown/GuestInvoke/Keyboard/MouseClick/Close ; **aucune restauration, création de checkpoint, attache ISO, suppression de VM, modification hôte, tâche ou service**. Nouveau pipe privé ; les helpers du kernel utilisent désormais cette autorité, pas l'ancien canal expiré.
- VM **Off réellement relu sous cette nouvelle autorité**, puis démarrage de ce seul guest. Payload **`native-only-continuation.ps1` SHA `39611af7ded20393d30b5ec41bacee943c29a735529a2d8df061a79832302b04`**, entrée **`3664748883d30b2f5c60501e6e90fffc72fbc58cec47ca36f472fec4615087ec`** : copies guest vérifiées, parsers **0 erreur**. Aucun scénario natif encore lancé par cette nouvelle entrée.
- Nouvelle preuve prévue **`<racine>/native-proof-2`**, TEMP **`<racine>/mcw-tmp`** ; refus si ces répertoires ou marqueur existent. **51 empreintes des deux tentatives précédentes**, vraies commandes préparatoires **0**, MSI et deux binaires figés contrôlés ; aucun reçu effacé/réécrit. Même source produit **30051ce…**, fix des chemins **615dd3c**. **Pas de cargo-build, Wix-build ou cargo-install à rejouer**, uniquement le consommateur natif original avec ses contrôles réels et preuves MSI complètes.
- Reprise immédiate : **Winlogon QA normal puis lancement depuis son Explorer**, inventaire et contexte réels revérifiés ; attendre/lire les reçus, préserver en cas d'échec, arrêter le guest avant expiration. **NOT READY FOR RELEASE** jusqu'au résultat effectivement obtenu. Les preuves durables et l'autorisation de nettoyage conditionnelle restent conservées ; ne supprimer aucune VM tant qu'une reprise est nécessaire.

### Vrai tray standard qualifié — MSI installé, lecture cachée du test corrigée

- Entrée Explorer native seule à **08:49:36Z**, reçu démarré **08:50:31Z** ; **51 fichiers antérieurs inchangés**, source compilée **30051ce…**, aucun cargo-build/WiX-build/Cargo-install rejoué. Le TEMP aplati a passé le premier cas auparavant bloqué par WinError 206.
- Reçus natifs réels **positifs** : suppressions différées des deux binaires **sans frontend**, avec **compagnon** et avec **frontend console** ; **vrai tray initialisé en session standard interactive**, autostart possédé retiré coopérativement ; refus de worker occupé sans toucher au frontend/autostart ; refus des compagnons modifiés, reçu inconnu, hardlink et protection demandée ; propriété/restauration native **zéro matériel** conservée. Ce ne sont pas de simples parsers ou observations de runner administrateur.
- **Installation MSI réelle ExitCode 0**, per-user, produit **`{208FBC0C-A247-4CDF-872A-486FB6FA9215}`**, avec instantané du PATH utilisateur antérieur enregistré. **Même installation encore présente** sous le sandbox possédé ; pas de seconde installation ni désinstallation incertaine.
- Le natif a ensuite échoué **08:53:29Z**, après **68 commandes enregistrées**, car son consommateur d'application MSI s'est arrêté **avant son répertoire de preuves** : **`windows-application-native.ps1:31`, Get-Item sans `-Force` ne lit pas le dossier caché AppData**. L'absence de `cleanupSafe` a correctement empêché la suppression MSI. **Ce n'est pas un échec d'installation MSI ni un refus du tray.**
- Correction identique aux conventions déjà présentes dans le helper/contexte : **Get-Item -LiteralPath … -Force**, dans **windows-application-native.ps1** et son autre lecteur d'ancêtres **windows-brand-native.ps1**. **Le test ReparsePoint, les limites de racine, le contexte standard et les ACL restent inchangés** ; Force concerne ici la lecture des éléments cachés, pas une suppression ou un contournement de permissions.
- Smoke réel PowerShell 5.1 sur les **deux boucles AST originales corrigées** : **ancêtre caché réellement créé/accepté**, **jonction réelle possédée refusée dans les deux consommateurs**, **ExitCode 0**, aucun produit/politique exécuté ou modifié. Preuve **`actual-hidden-ancestry-boundaries-result.json`**. Aucun test natif déjà réussi à relancer.
- **175 fichiers du natif/commandes/workers/MSI-install exportés et SHA-256 byte-vérifiés** dans la preuve durable **`retained-native2-portable-success-msi-installed-hidden-path-failure.json`**. Les deux échecs précédents restent intacts ; le nouveau reste **failed**, pas converti en PASS.
- **Prochaine étape : qualifier exactement ce MSI déjà installé avec le consommateur corrigé dans un nouveau répertoire**, puis exécuter les assertions originales de délégation/désinstallation MSI, conservation des données/PATH et réelle désinstallation Cargo. Ne pas rejouer les scénarios portables réussis, installer un second produit ou ignorer le contrôle `cleanupSafe`. VM conservée pour cette continuation ; autorité PID26092 toujours bornée à **10:34:48Z**, nettoyage destructif encore interdit tant qu'une reprise est nécessaire. **NOT READY FOR RELEASE** avant ces dernières preuves.

### Suite ciblée du même MSI et Cargo — payloads vérifiés avant exécution

- Fix des deux lecteurs cachés sauvegardé/poussé **`d276555c55cde1adcae46af3c1213d14c10aff7d`** ; smoke contexte/parsing/C# également **PASS**, toujours qualification produit non exécutée par ce smoke.
- Opérateur privé **`msi-cargo-continuation.py` SHA `a505795b18bee656a08f14b0f721baa2b93f437eb341a4e80d61cb978285fd33`**, données **`9fe93ed86f9d17003f081d0a4a2724e43e5a3d37d81ed1027462fc75e06285fc`**, entrée Explorer **`1dd1a1b76afcaa3fb2a465e7ec289e17bb9862867ac45f4a193f12d1730de146`**. Payloads guest SHA-vérifiés, parseurs Python **3.13** et PowerShell **0 erreur** ; aucune exécution de fixture sous remoting.
- Consommateur application corrigé **`637529ebc443488c8eedc4d9d6b169dfd3b6643d451458ca44fd31c7d7896df2`**, placé dans le dossier privé **msi-app-corrected**, avec le helper contexte inchangé **19f3ca1d…**. Le checkout produit **30051ce…** reste intact.
- L'opérateur fixe **226 empreintes des anciens reçus/fichiers**, réutilise **le ProductCode, le vrai répertoire installé et le snapshot PATH d'avant installation issus du registre de commandes réel**, pas d'un relevé après installation. Il appelle **le vrai processus application** puis les instructions AST originales de vérification/délégation/désinstallation MSI et le consommateur Cargo original. Aucun faux `CompletedProcess`, copie d'installation Cargo, second MSI ou succès hérité d'un échec.
- Nouveau répertoire attendu **`msi-cargo-proof-3`** ; le résultat sera explicitement **un agrégat de continuation** référençant les cas portables réellement réussis, pas un PASS réécrit des trois tentatives précédentes. **Les exigences du même MSI, standardUserQualified, requireStandardUser, cleanupSafe, données et PATH restent obligatoires avant toute suppression par le gestionnaire.**
- **Prochaine action : lancer l'entrée depuis l'Explorer QA actuellement visible**, sans rejouer compilation/installation/cas portables, puis inspecter les vrais reçus et captures. En cas de nouvel échec, préserver ce MSI et toute preuve au lieu de forcer une désinstallation. VM non supprimable tant que la qualification n'est pas terminée ; autorité finie toujours **10:34:48Z**.

### Fenêtre native standard réellement affichée — barrière des raccourcis à qualifier

- Entrée réelle de suite MSI/Cargo **09:18:58Z** ; même produit, aucune réinstallation. Le consommateur corrigé a confirmé **standardUserQualified=true**, **RequireStandardUser=true**, **WinSta0/Default visible**, profil/SID QA réel et jeton non administrateur sans restriction.
- **Réels succès d'application observés avant l'échec** : cinq démarrages GUI à froid donnent un seul owner ; lien Start Menu authentique et AUMID **MicCamWatch.MicCamWatch** ; ouverture depuis le lien ; fermetures masquant la fenêtre et réouvertures séquentielles **avec le même PID6592, tray HWND656066 et desktop HWND459300**. Aucun événement console parasite. Image native réellement examinée : **920×720**, anglais/thème clair, SHA **`298b55d416fb261ce1fdbcf8be04ed81b7546d2d454f48959f4bb160d2f2f0a9`**, sauvegardée durablement **`installed-start-menu-real-standard-window.png`**. Pas une preuve de toutes les langues/thèmes ni du regroupement de la barre des tâches.
- Échec de l'assertion **« exactement un processus résident »** dans le burst concurrent GUI/CLI/raccourcis ; le snapshot fautif des résidents n'était pas enregistré. **L'origine de tout processus supplémentaire n'est donc pas encore prouvée.** Le nombre/HWND du tray propriétaire avait passé ses assertions précédentes. Le cleanup coopératif de l'owner et de tous les descendants a réellement réussi, **cleanupSafe=true**, mais **status=failed** ; le MSI reste conservé, pas supprimé sur ce résultat.
- Défaut de synchronisation établi dans le harness : les quatre **Start-Process du .lnk réel** ne fournissaient aucun handle à **Wait-Launchers**, qui attendait uniquement les entrées GUI/CLI explicites. Le comptage pouvait donc précéder leur fin. **Hypothèse pour cet échec, pas un second owner produit démontré.**
- Correction ciblée **windows-application-native.ps1** : lancer les **mêmes raccourcis réels via Start-App / ShellExecute**, conserver les vrais handles et exiger aussi leur **sortie 0** avant le comptage. **Aucun délai arbitraire, relaxation du nombre de résidents/HWND, ou changement du produit Win32.** En cas d'autre refus, sauvegarder désormais le snapshot exact résidents/fenêtres avant de lever la même exception.
- **Test réel de cette barrière encore à exécuter**, dans un nouveau proof/input/point d'entrée, en conservant l'échec précédent. Les anciennes étapes portables, compilation, MSI-install et Cargo-install ne sont pas à refaire. **NOT READY FOR RELEASE**, VM/MSI conservés ; aucune suppression de nettoyage à ce stade.

### Qualification Windows standard terminée — PASS réel, preuves durables vérifiées

- Entrée finale **09:38:52Z**, terminée **09:40:03Z / ExitCode 0**. Agrégat explicitement étiqueté **explicitly-approved-staged-native-aggregate / passed**, source compilée **`30051ce70e904912d988d17ad8e72cf4d19c6568`**, consommateur corrigé **`bebad88aeac245fc286a52b3c8b23871654e2430`**. Les cas portables réellement réussis sont référencés, pas rejoués ; les **244 anciens fichiers de preuve restent SHA-inchangés**, aucun échec converti en succès.
- **Même MSI réel : passed / standardUserQualified=true / requireStandardUser=true / cleanupSafe=true**. Raccourci/AUMID réel, owner et HWND retenus, fermeture masquant la fenêtre, réouvertures séquentielles/concurrentes GUI/CLI/.lnk et démarrages de fond ont passé les assertions inchangées. Tous les launchers Shell sont maintenant réellement suivis/attendus et doivent sortir **0**, pas un délai ajouté pour masquer une concurrence.
- Refus d'auto-update du MSI **ExitCode 2 en 335 ms**, **0 connexion proxy / 0 staging**, payload/registre/raccourci/PATH/données inchangés et même owner/HWND. Puis **vraie délégation au ProductCode exact et `msiexec /x` ExitCode 0**, données et ordre/type du PATH non développé conservés ; **vraie délégation Cargo et `cargo uninstall` ExitCode 0**, données conservées. Aucun contrôle de périphérique physique ou installation hôte impliqué.
- **45 fichiers de preuves finales** exportés byte-for-byte et SHA-vérifiés dans **PASSED-final-windows-standard-msi-cargo-proof.json**, plus reçus finaux lisibles, captures PNG et reproduction exacte, hors Temp dans **`%LOCALAPPDATA%\MicCamWatchQualification\0.21.0-20261026-c1c5de53`**. Archives des échecs d'origine/33/175/18 fichiers conservées ; métadonnées utiles ISO/prérequis/consentements préservées, jeton de transcription ancien explicitement expurgé, mots de passe/DPAPI exclus.
- **Vrais binaires et MSI de test** récupérés par segments sans réexécution et relus/re-SHA-vérifiés dans **EVIDENCE-only-tested-source-30051ce-artifacts.zip**, **9 651 330 octets**, SHA **`8f023186bfbcd7e9cb917cdaf9e21de85fa3ace6bfbb43a50d0339de76af1a54`**. Ce ZIP privé n'est pas une release. Statut Authenticode des trois artefacts réellement lu : **NotSigned**, sans supposition sur les futurs assets publics.
- **CI ciblée [40301860252](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40301860252) PASS**, source exacte **bebad88…** ; artefact **12641150676** téléchargé et lu : quatre scripts/types, drapeau natif un octet, refus admin/modes mixtes corrects, **qualificationStatus=not_executed**. Ce smoke n'est pas présenté comme le test natif local. Les cinq cibles produit restent qualifiées sur **4b90fd6…**, sources produit/build/installateurs inchangées ; aucun test matériel ARM inventé.
- Manifeste durable **qualification-complete-manifest.json**, **24 fichiers relus/checksummés**, SHA **`6fe7b099796bf745a3cda8615d7a92ea263d8542fa3a5f70fad64c3eb3c2c472`** ; contient les limites, succès et autorisation conditionnelle de nettoyage. ISO utilisateur originale toujours présente, **8 225 329 152 octets**, aucune nouvelle copie.
- **09:56:43Z** : ProductState **-1**, aucun raccourci test, aucun binaire Cargo test et aucun processus MCW actif ; aucune désinstallation hôte. **VM Off réellement relu à 09:57Z**, puis Close du contrôleur et **PID26092 réellement absent à 10:03Z**. Aucun propriétaire élevé concurrent à laisser actif.
- **READY FOR RELEASE dans le périmètre logiciel documenté ; publication toujours non autorisée.** Anglais/clair standard réellement affiché ; autres langues/thèmes, macOS/Linux et limites physiques conservent leurs preuves/scopes antérieurs. Packaging/signature/provenance des futurs assets, autorisation publication et étapes du plan de release restent distincts ; aucun main/tag/release/déploiement.
- **Prochaine action sûre : nettoyage conditionnel autorisé**, avec nouvelle UAC ponctuelle bornée à **cette VM et ses seuls fichiers possédés**, manifeste/vrais PASS vérifiés, VM Off et anciennes autorités closes avant toute suppression. Préserver ISO, preuve durable, worktrees/branches/cinq exclusions historiques, installations MicCamWatch et caches globaux. Mesurer avant/après les suppressions ; ne pas prétendre un gain d'espace tant qu'il n'est pas observé.

## 2026-10-26 — Reprise après interruption et nettoyage final terminé

- Reprise sur **release/0.21.0-candidate**, HEAD/origin **`7324806021d23daf765b4f35a0ed86157c28bf01`**, arbre propre. Le kernel précédent n'était plus disponible : l'état a été reconstruit depuis Git et les reçus durables, **sans rejouer les tests Windows réussis**.
- À **11:17:59Z**, l'ancienne opération de nettoyage n'avait **pas démarré** : aucun `cleanup-intent.json`, aucun `cleanup-result.json`, aucun processus exécutant le script. L'inventaire Hyper-V non élevé a explicitement refusé l'accès ; **ce refus n'a pas été interprété comme une absence de VM**.
- Manifeste **6fe7b099…** et **ses 24 fichiers** relus/SHA-256 revérifiés ; vrais reçus natif/application **passed**, `standardUserQualified`, `requireStandardUser`, `cleanupSafe`, suppressions gestionnaires MSI/Cargo et conservation des données/PATH confirmés. Aucun défaut produit supplémentaire, compilation, réinstallation ou scénario natif relancé.
- Nouvelle **UAC ponctuelle** du script de nettoyage conditionnel déjà autorisé, processus **10460 / ExitCode 0**, terminé **11:20:24Z**. Avant suppression : nom/ID exacts **MCW-021-QA-c1c5de53 / 260b15b3-0281-4520-aa6b-7ca208a01d26**, état **Off**, chemins de VM et de toute la chaîne VHD/checkpoint strictement dans le répertoire possédé, aucune référence par une autre VM, aucune autorité précédente active, vrais PASS et chaque fichier durable vérifiés.
- **Nettoyage réellement terminé** : enregistrement de cette seule VM supprimé, VHDX/AVHDX et répertoire privé **mcw-021-eval-20261025-ngbjedk9** supprimés (**315 fichiers** dans l'inventaire préalable). La jonction synthétique connue a été déliée **sans suivre sa cible** ; tout autre reparse aurait bloqué l'opération. Cinq seuls dossiers temporaires identifiés — trois smokes locaux et les deux anciens téléchargements de reçus CI — également supprimés. Le compilateur, les téléchargements et caches **internes à cette VM** ont disparu avec son disque ; **aucun cache global de développement hôte supprimé**.

| Mesure C: pendant le nettoyage | Espace libre (Gio, base 1024) |
| --- | ---: |
| Avant, 11:19:36Z | 133,651 |
| Après, 11:20:23Z | 181,192 |
| Gain réellement mesuré | **47,540** |

- Reçu durable **`cleanup-result.json` : completed, vmRemoved=true, missionRootRemoved=true, officialIsoRetained=true, error=null**. Gain exact **51 045 969 920 octets** ; les observations antérieures utilisateur/08:26 ne sont pas utilisées pour attribuer un gain artificiel au nettoyage. Les ensembles d'IDs des autres VM, comparés avant/après, sont identiques.
- Vérification après suppression : répertoire de mission réellement absent, **les 24 preuves toujours SHA-identiques** dans **`%LOCALAPPDATA%\MicCamWatchQualification\0.21.0-20261026-c1c5de53`**, archives des anciens échecs et ZIP des vrais artefacts conservés. ISO officielle utilisateur toujours présente, **8 225 329 152 octets**, taille/dernière modification inchangées, aucune copie créée. Les identifiants de récupération et contrôleurs temporaires n'ont plus d'utilité puisque cette VM qualifiée a été supprimée.
- Worktrees/branches conservés ; **origin/main toujours dc40e95**, GUI/ARM **cafd98d**, documentation **5ee9d49**. Les cinq exclusions historiques de l'arbre original sont inchangées : **.wrangler/**, **WATCHDOG.yml**, **debug_macos/**, **docs/next_projects.md**, **installer/tests/__pycache__/**. Aucune installation MicCamWatch personnelle, protection micro/caméra, configuration de sécurité, compte Windows hôte ou autre VM modifié.
- **Point de reprise final : aucun test Windows bloquant ni travail VM restant. READY FOR RELEASE dans le périmètre logiciel documenté**, avec limites physiques/manuelles et artefacts locaux **NotSigned** inchangés. **Ne pas recréer la VM ni rejouer la qualification par défaut.** La prochaine opération est une décision utilisateur distincte sur la préparation/validation des futurs packages et leur publication ; **aucun main/tag/release/déploiement autorisé ou effectué** par cette clôture.

## 2026-10-26 — Packages 0.21.0 non publiés : préparation autorisée

- Nouvelle autorisation utilisateur limitée à **produire/vérifier les packages et préparer les opérations futures**. **Main, tag, release GitHub et déploiement du site restent interdits** ; stable public réellement relu **v0.20.0**, neuf assets, non-draft/non-prerelease. Aucun nouveau test VM ni installation hôte autorisé ou nécessaire.
- État préalable propre : **release/0.21.0-candidate / ff0f05ceba6506dc4cffd0ca54f117780b54511e**, égal à origin ; tag **v0.21.0 absent** ; **origin/main dc40e95bc9c1c6a14386eb5e773f01b62b47aa71** inchangé, ancêtre du candidat (74 commits). Main n'est pas protégé selon l'API : ce n'est **pas** une autorisation de le modifier. Autres worktrees et cinq exclusions historiques préservés.
- Vérification des gardes : Release dispatch sur une **branche non-tag** produit `verified-release-bundle`, mais **Publish complete GitHub release exige refs/tags/**. Le workflow réutilisable ne publie pas de release ; le workflow du site n'est pas déclenché par ce dispatch. Variable **WINDOWS_RELEASE_APPROVED=true** relue ; environnement **release** : délai d'une minute, aucun required reviewer ni restriction de branche observé. Ces réglages ne valent pas autorisation de publication.
- Run réellement lancé et identité relue : **[40316172507](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40316172507), attempt 1, workflow_dispatch, refs/heads/release/0.21.0-candidate, source exacte ff0f05ceba6506dc4cffd0ca54f117780b54511e**. Watcher local passif ; ne pas relancer ce run par défaut en cas d'interruption.
- Preuves nouvelles hors Temp : **`%LOCALAPPDATA%\MicCamWatchQualification\0.21.0-packages-20261026-ff0f05c`**, admission et `package-run.json`. Les dix fichiers/noms et neuf sujets manifestés/attestés sont figés avant téléchargement. **À ce checkpoint, aucun succès de build, digest, signature ou attestation finale n'est encore annoncé**.
- Deux lots indépendants : cohérence des notes et audit read-only des gardes/provenance. Correction documentaire du statut d'autorisation et du gate Windows clos ; la seule phrase du corps des futures notes workflow précise que **MSI upgrade/repair**, pas le scénario standard déjà passé, reste non validé. Aucun changement de code, triggers, jobs, sécurité ou site.
- **Point de reprise : laisser finir le run existant, inspecter chaque job/reçu, télécharger son bundle, exiger dix fichiers/neuf entrées SHA exactes, vérifier les neuf provenances sous contraintes repo/workflow/source/ref/hosted et lire les signatures Windows ainsi que les preuves macOS réelles.** Conserver les résultats de branche séparés des futurs octets du tag ; bloquer toute publication si incomplet ou incohérent.

### Packages réellement produits et vérifiés — attente de l'accord final

- **Run Release 40316172507 / attempt 1 : success**, sur **ff0f05ceba6506dc4cffd0ca54f117780b54511e**, branche non-tag figée. Windows et les quatre cibles Unix ont réussi ; le job d'assemblage nommé `publish` a réussi mais **son étape Publish complete GitHub release est réellement skipped**. Le job Windows du reusable est volontairement skipped (`native-only`), pas substitué à la **CI complète 40315147426, cinq jobs success sur le même ff0f05c**, également conservée.
- Artefact **verified-release-bundle / 12647996675** réellement téléchargé : ZIP SHA **03cae6483c66d06fdf6ee6983a25ae41fa4b95807e2b7418158b4e6c8fe7bb6e**, identique au digest API ; inventaire exact **dix fichiers non vides**, **neuf entrées uniques SHA256SUMS toutes correctes**, aucun self-hash. Les dix empreintes complètes sont dans **docs/RELEASE_0.21.0.md**, section **Unpublished packages verified**. Ce ne sont pas les empreintes anticipées du futur tag.
- **Neuf provenances effectivement vérifiées**, contraintes repo **Roman-Cuisset/miccamwatch**, workflow **release.yml**, signer/source **ff0f05c complet**, ref **refs/heads/release/0.21.0-candidate**, SLSA v1 et refus self-hosted. Métadonnées du certificat GitHub OIDC/run attempt et preuve de transparence relues ; bundle signé conservé. Six premiers sujets PASS ; trois erreurs initiales **d'initialisation du vérificateur Sigstore**, puis un retry sérialisé des seuls trois échecs **avec exactement les mêmes contraintes**, tous PASS. Échecs initiaux conservés ; cause exacte non prouvée, aucun cache global effacé ni garde de confiance désactivé.
- **Windows final : NotSigned** pour les deux EXE extraits et le MSI, sans signataire ni timestamp. Receipt portable format 1/version 0.21.0 et deux hashes vérifiés. Inspection **lecture seule** de la base MSI/cabinet : ProductVersion **0.21.0**, ProductCode **{EB131A62-5BA6-4958-8D99-E8E51094A3B7}**, un raccourci MicCamWatch `--open`, deux payloads **byte-identiques à ceux du ZIP**. Aucun msiexec, EXE ou tray candidat exécuté sur l'hôte personnel. Le smoke d'exécution/installation des packages relève du runner hébergé ; le PASS standard local reste son propre build antérieur de code applicatif inchangé.
- **Linux x64/ARM64** : deux payloads par archive, ELF64 little-endian, machines **62/183**, quatre hashes identiques aux rapports natifs readelf de ce run. Baseline **glibc 2.35 / PipeWire 0.3.48**, symboles GLIBC observés jusqu'à **2.34**, sans abaisser le support annoncé. Reçus GTK **21 cas PASS par architecture**, sept langues × clair/sombre/contraste élevé, drapeaux matériel physique false ; scénarios user/helper root install/removal lus Exit 0. **Aucune certification Raspberry Pi/SBC, CSI ou périphérique physique ajoutée.**
- **macOS Intel/Apple Silicon** : Mach-O minces vérifiés, vraies étapes package `hdiutil verify`/scellage consultées dans leurs logs, reçus copied-DMG **passed**, version 0.21.0 et **codesign deep/strict Exit 0** pour les deux apps. **Ad-hoc seulement**, ni Developer ID, notarisation, timestamp de confiance ou clearance Gatekeeper. Aucun montage/codesign macOS sur Windows et aucune nouvelle validation visuelle humaine inventée.
- **SBOM réel SPDX 2.3 / Syft 1.42.3** : **503 packages, 2 246 relations**, miccamwatch 0.21.0 présent. Inventaire du dépôt généré sur la source d'assemblage, pas audit complet de chaque binaire. Les quatre ZIP de preuves natives ont aussi été téléchargés et leurs **digests ZIP API vérifiés**.
- Preuves durables nouvelles : **`%LOCALAPPDATA%\MicCamWatchQualification\0.21.0-packages-20261026-ff0f05c`** : bundle exact, manifeste, ZIP source/API, quatre archives natives, logs réels Windows/macOS, reçus de lecture des signatures/MSI/ELF/scellages et neuf vérifications de provenance (premiers échecs/retries inclus). Corrections documentaires sauvegardées sur la branche candidate au checkpoint **05234ed** après lancement ; elles ne sont pas présentées comme la source des octets ff0f05c.
- Plan final enrichi sans l'exécuter : vérifier le mode d'auto-déploiement Cloudflare **avant** une future intégration main ; fast-forward seulement si la base vérifiée est toujours ancêtre, sinon préserver/réexaminer les nouveaux travaux. **Le push du tag déclenche directement la publication**, sans draft/reviewer garanti : exiger l'accord explicite pour les deux. Refaire les vérifications des vrais octets/attestations du futur tag, sans réutiliser à tort la provenance de branche.
- Site **inchangé sur 0.20.0**. Après release publique vérifiée et autorisation site distincte : épingler version/source du vrai tag/digest du script, ajouter le lien Linux ARM64 avec sept traductions et ses limites, tests/export puis déploiement approuvé ; préserver privacy/SEO/routes/icônes. Digest du script source candidat calculé **ef5dc62e71659261de0ad79b6b558e7d2446106d3adaed6019c27e651ce454bd**, à recalculer si son contenu change.
- **Point de reprise : packages de branche qualifiés, aucun blocage de vérification restant ; attendre l'autorisation finale main + tag/publication automatique + mise à jour/déploiement site.** Limites ARM physique, Windows 10, MSI upgrade/repair, autres vrais desktops/DPI/accessibilité et contrôles physiques inchangées. Ne recréer ni VM ni qualification déjà réussie ; ne pousser aucun tag, main ou release sous la seule permission de préparation.

## 2026-10-26 — Intégration main autorisée, publication toujours interdite

- Autorisation utilisateur précise : intégrer **00a85d1ddbbeaec93bcccf95106139bb5560cb84** après contrôle des automatismes, puis qualifier la révision intégrée. **Aucun tag v0.21.0, aucune release ni mise à jour/déploiement du site autorisé.** Références relues : candidate/origin **00a85d1**, main **dc40e95**, ancêtre valide ; worktrees conservés, arbre candidat initialement propre.
- **Risque réel trouvé avant mutation** : le précédent main possède un vrai check **Cloudflare Pages**, app **cloudflare-workers-and-pages**, projet **miccamwatch**, déploiement **45aed629-22d4-403c-80ac-48b7383d74d7**. L'absence de webhook classique ne prouve pas l'absence de GitHub App. Un push direct non gardé serait donc incorrect.
- Solution bornée sans modifier les réglages du fournisseur : commit de documentation/notes descendant du candidat, titre commençant par **[CF-Pages-Skip]**, puis avance rapide de main vers ce commit. [Documentation Cloudflare officielle](https://developers.cloudflare.com/pages/configuration/git-integration/github-integration/#skipping-a-build-via-a-commit-message) : ce préfixe omet le déploiement sans configuration ; absence de check Pages attendue en cas de skip. [GitHub Actions utilise d'autres marqueurs](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/skip-workflow-runs), aucun utilisé : **CI complète toujours active**. Aucun accès OAuth, changement de configuration Cloudflare ou secret nécessaire.
- Workflows relus : Release ne se déclenche pas sur push main et son action de publication exige **refs/tags/** ; aucune chaîne workflow_run de publication. Le workflow site sur main **teste/exporte/uploade un artefact**, sans commande de déploiement. Le pin **site/release.json reste 0.20.0** ; le builder candidat sait récupérer le script **du source public épinglé et vérifié**, au lieu de diffuser le nouveau script 0.21 accidentellement.
- Baseline HTTPS réellement capturée avant main : **13 URL en 200**, huit accueils pointant tous vers **v0.20.0**, privacy EN/FR, robots/sitemap et **/install.sh SHA b7348c9a6f37a3d4f89faa2b767fd4880162d90c8f5d7c151185e0823384d1bb**. Premier transport Python : timeout lecture, aucune mutation ; curl natif IPv4 a capturé les URL restantes, sans ignorer TLS ou transformer ce timeout en succès.
- Encadré **Signing and hardware validation — important** ajouté au début des notes Markdown et du corps des futures notes GitHub : Windows **NotSigned** dans la préparation réellement vérifiée ; macOS **ad-hoc, sans Developer ID/notarisation** ; ARM64 hébergé natif **pas une validation Raspberry Pi/SBC/périphériques/contrôle physique**. Aucun changement de code applicatif, packaging, triggers, permissions ou protections.
- Preuves de cette phase : **`%LOCALAPPDATA%\MicCamWatchQualification\0.21.0-main-integration-20261026`**, admission conditionnelle, baseline publique et fichiers HTTP. Les anciens 44 fichiers de preuve de packages et leur manifeste **c0bc529a…** restent intacts. **Point de reprise à ce checkpoint : sauvegarder le commit gardé, relire refs/ancestry/titre, pousser uniquement ce descendant gardé vers main, identifier les checks main exacts puis comparer production.** Ne pas confondre préparation et intégration réellement exécutée.

### Main intégré et qualifié — commit à taguer figé, attente de l'accord final

- **Intégration réellement effectuée** : avance rapide non forcée **dc40e95 → 3d74686ace7b04728794765f9eb57c0c0a8bc84c**. Le candidat approuvé **00a85d1ddbbeaec93bcccf95106139bb5560cb84** est ancêtre, intégralement conservé ; seul descendant ajouté avant main : encadré de limites/autorité documentaire avec titre **[CF-Pages-Skip]**. Ref main seule poussée explicitement, `push.followTags=false` limité à cette commande. Aucun reset/rebase/force-push, tag, autre worktree ou fichier historique supprimé.
- **Main reste figé à 3d74686ace7b04728794765f9eb57c0c0a8bc84c : c'est le commit exact proposé pour v0.21.0**, pas le HEAD d'un compte rendu ultérieur. Après qualification, les mises à jour de bilan sont commitées/poussées **sur la branche candidate seulement**, afin de ne pas déplacer la source main testée ni relancer artificiellement une qualification documentaire.
- **[CI main 40325121017](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40325121017), source exacte 3d74686… : cinq jobs success**, format/Clippy/tests/builds et scénarios natifs prévus. Cinq ZIP de preuves natives téléchargés, leurs digests API vérifiés, reçus lus : Windows outer/application **passed/cleanupSafe=true**, **requireStandardUser=false/standardUserQualified=false** sur ce runner (l'ancien vrai test standard local reste distinct) ; Linux ELF 62/183 et deux payloads par cible concordants, **21 cas GTK PASS par arch** ; macOS Intel/ARM deux app-smoke **passed**, scellages deep/strict **Exit 0**. Aucune nouvelle validation visuelle humaine, certification physique ou faux standard-token annoncé.
- **[Native Unix installation main 40325121049](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40325121049) : trois jobs success**, Linux x64/macOS Intel/ARM, vrai script HTTPS épinglé sur le commit intégré avec assets publics **v0.20.0**. Logs réellement lus : refus sûrs, conservation executable/receipt/PATH et bootstrap/update macOS public. Trois petits ZIP de preuves également API-digest vérifiés. **Pas de test ARM public 0.21 inventé avant la publication** ; ARM natif candidat qualifié séparément par CI.
- **[Static website main 40325120956](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40325120956) : success**, artefact **12650319640** téléchargé/checksummé contre l'API. Huit accueils générés gardent les liens **0.20.0**, canonical/og:site_name/WebSite corrects ; huit privacy gardent **miccamwatch@gmail.com** ; script exporté **SHA b7348c9a…**, pas le nouveau script de développement. **Artefact Actions seulement, aucun déploiement**.
- **Vérification publique finale** : treize URL HTTPS **200, byte-identiques avant/après main**, huit accueils toujours en 0.20.0, privacy EN/FR, robots/sitemap et install.sh. Le hash exact du script public reste **b7348c9a6f37a3d4f89faa2b767fd4880162d90c8f5d7c151185e0823384d1bb**. Absence de check Cloudflare conforme au skip officiel ; aucun Release déclenché pour cette source. API : **tag/release v0.21.0 absents**, latest public **v0.20.0** inchangé.
- Relu : **WINDOWS_RELEASE_APPROVED=true**, environnement release **timer une minute, pas de required reviewer**. Ce n'est pas un accord humain pour publier. Le futur push de tag démarre directement le build/publication ; le préfixe Cloudflare ne bloque ni CI ni Release. Encadrés bien visibles dès le début des notes Markdown/GitHub : Windows **NotSigned** dans le bundle vérifié, macOS **ad-hoc/sans Developer ID ni notarisation**, Linux ARM64 **hébergé natif non certifié sur Raspberry Pi/SBC/périphériques physiques**. Autres limites Windows 10/MSI upgrade-repair/DPI/accessibilité/desktops réels inchangées.
- Preuves nouvelles durables **0.21.0-main-integration-20261026** : admission/commande push réelle, comparaisons YAML à comportement identique hors corps des notes, métadonnées CI/site/install, huit archives API vérifiées de preuves natives/installateurs, export site et captures HTTP avant/après. Les **44 preuves du bundle précédent restent SHA-identiques**, source **ff0f05c**, pas requalifiées comme les futurs octets du tag. Aucun hôte personnel installé/désinstallé, nouveau contrôle micro/caméra, VM recréée, compte/droit ou réglage fournisseur modifié.
- **Prochaine action : attendre l'autorisation explicite pour créer/pousser v0.21.0 sur le SHA main figé, publier les dix assets stable/latest, puis actualiser/déployer le site après vérification des vrais fichiers publics.** Avant ce futur push, relire refs/version/autorisation et conserver les limites ; refaire SHA/signatures/provenance **refs/tags/v0.21.0** sur les nouveaux fichiers, jamais substituer les attestations de branche. **Aucun tag, release ou site 0.21 créé pendant cette mission.**

## 2026-10-26 — Publication stable 0.21.0 autorisée, tag officiel poussé

- Autorisation finale utilisateur : publier **v0.21.0 exclusivement sur 3d74686ace7b04728794765f9eb57c0c0a8bc84c**, puis actualiser/déployer le site seulement après vérification complète des vrais packages publics. **Arrêter les étapes suivantes sur échec critique, asset manquant, SHA incorrecte ou provenance incohérente ; ne jamais remplacer silencieusement un fichier publié.** Installations/protections personnelles toujours hors périmètre.
- Préflight réellement relu : origin/main au SHA exact ; CI **40325121017**, Unix **40325121049**, site **40325120956** success sur cette source ; Cargo **0.21.0** ; variable Windows **true** ; environnement release timer une minute ; aucun tag local/distant ni release 0.21 préexistants. Arbre propre et travaux protégés conservés.
- **Tag annoté créé et poussé** sans force, refspec tag seul et followTags désactivé pour la commande : objet **06d800ba536b5e6945abe4a4345957ba85c79cbf**, cible déréférencée vérifiée **3d74686ace7b04728794765f9eb57c0c0a8bc84c**. Ce n'est pas le HEAD documentaire 826f31d.
- **[Release 40331094727](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40331094727)** réellement identifié : event **push**, headBranch **v0.21.0**, source exacte autorisée. Watcher passif actif. **À ce checkpoint, le run est en attente/en cours : aucun succès de publication, hash public ou signature finale n'est encore revendiqué.**
- Preuves hors Temp : **`%LOCALAPPDATA%\MicCamWatchQualification\0.21.0-publication-20261026`**, admission/CI, création et sortie réelle du push, identité du run. En cas d'interruption, lire ce run existant ; **ne pas recréer le tag, rejouer la publication ni écraser un asset**.
- Deux lots indépendants préparent le site sept langues et les références documentaires, **sans édition tant que le parent n'a pas ouvert PACKAGE_GATE_PASSED**. Parent propriétaire des SHA/provenances/signatures réelles, de ce document, du plan de release et des pushes/déploiement.
- **Point de reprise : laisser finir Release, vérifier les dix vrais fichiers publics contre API et neuf entrées manifestées, leurs neuf attestations contraintes à refs/tags/v0.21.0/source/workflow/hosted, architectures et signatures ; confirmer public stable/latest et encadrés d'avertissements.** N'ouvrir le site que si ce gate passe. Ne pas transformer les hashes/provenances de branche ff0f05c en preuves des nouveaux fichiers du tag.

### Publication interrompue — GitHub create-release HTTP 500, aucun package 0.21 public

- **Résultat réel de [Release 40331094727](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40331094727) : failure.** Les cinq jobs Windows x64, Linux x64, Linux ARM64, macOS Intel/Apple Silicon sont **success**, ainsi que l'assemblage des dix fichiers, les neuf SHA du manifeste, la génération de provenance et l'upload du bundle Actions. Le job publish échoue ensuite, pas les compilations du produit.
- **Erreur démontrée :** l'action de création de release reçoit **HTTP 500 trois fois**, à **14:28:30Z / 14:28:31Z / 14:28:33Z**, puis **Too many retries**. Journal complet `failed-publish-job.log`. La cause interne de ces réponses GitHub n'est pas établie ; ne pas affirmer une panne générale du service ni inventer une correction produit.
- **État relu après l'échec :** aucune release/draft v0.21.0 dans l'API authentifiée, aucun asset 0.21 public ; **latest reste v0.20.0**. Main distant reste **3d74686ace7b04728794765f9eb57c0c0a8bc84c** ; tag annoté **06d800ba536b5e6945abe4a4345957ba85c79cbf**, cible déréférencée identique. **Ne pas recréer, déplacer ou supprimer ce tag.**
- **Arrêt impératif appliqué :** aucun rejeu de Release/création manuelle, remplacement d'asset, changement main, cutover documentaire/site ou déploiement. Les deux agents ont confirmé **zéro édition** et PACKAGE_GATE_BLOCKED ; leurs audits sont sauvegardés dans `blocked-next-stage-audits.txt`. Les vérifications des vrais fichiers publics, signatures/provenances finales, parcours d'installation HTTPS 0.21 et sept langues déployées sont **bloquées/non exécutées**, pas réussies.
- **Bundle non publié sauvegardé :** [artifact 12659006675](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40331094727/artifacts/12659006675), ZIP **26 092 793 octets**, SHA-256 **4620f342554bc9ab905703af459939870934c200b5c588161fcecba6f4e193e1**, égal au digest API. Extraction contrôlée : dix noms attendus, neuf payloads/manifeste vérifiés, structures PE/ELF/Mach-O/UDIF et reçu Windows/SPDX inspectés avec le vérificateur existant. Source du rapport explicitement **3d74686… / refs/tags/v0.21.0**, jamais les métadonnées historiques ff0f05c.
- **Limites inchangées :** la génération de provenance du tag a réussi ; sa vérification indépendante et la lecture finale des signatures ne sont pas achevées. Aucun sceau/notarisation/compatibilité matérielle ARM supplémentaire revendiqué ; ARM64 reste validé en CI hébergée, pas sur Raspberry Pi/SBC physique. Les preuves antérieures conservent leur propre source.
- **Production protégée :** accueil HTTPS toujours relié aux téléchargements **0.20.0**, `install.sh` public SHA-256 **b7348c9a6f37a3d4f89faa2b767fd4880162d90c8f5d7c151185e0823384d1bb** inchangée. Aucun logiciel/protection micro-caméra de l'hôte modifié, aucune VM recréée, aucun autre worktree touché.
- **Preuves durables :** `%LOCALAPPDATA%\MicCamWatchQualification\0.21.0-publication-20261026` ; `failed-release-run.json`, `release-state-after-failure.json`, `failed-publish-job.log`, `unpublished-tag-bundle-12659006675.zip`, `unpublished-tag-bundle-verification.json`, `failure-boundaries.json` et audits des prochaines étapes. L'archive source immuable **codeload/commit 3d74686…** a SHA **4ccf67051daa4fc15ce52398fc96d61e3dcf7454b06202f763156fffcadc2d7c** ; l'archive de **tag** a une racine et une empreinte différentes : ne pas intervertir ces deux URLs/checksums dans le futur site.
- **Prochain point de reprise : décision explicite du propriétaire sur une reprise bornée de la publication GitHub**, après relecture des refs et de toute release/asset éventuellement apparu. Réutiliser les bytes attestés du bundle conservé plutôt que relancer les cinq builds réussis. L'action épinglée utilisait le défaut **overwrite_files: true** : **aucun rerun aveugle**, aucun écrasement silencieux. Vérifier tous les vrais fichiers publics et leurs preuves avant d'ouvrir à nouveau le gate site sept langues ; README/navigation/guides restent volontairement à la situation publique 0.20.0 jusque-là. `docs/RELEASE_0.21.0.md` contient le statut d'arrêt et les conditions de récupération.

### Reprise contrôlée autorisée — préflight avant une unique création de brouillon

- Nouvelle autorisation utilisateur : réutiliser **exactement** le bundle du run **40331094727**, sans recréer/déplacer le tag ni recompiler les cinq plateformes ; arrêter toute nouvelle tentative sur HTTP 500. Aucun logiciel/protection de l'hôte à modifier. Site toujours fermé tant que la release publique n'est pas entièrement vérifiée.
- API authentifiée relue : aucune release/draft **v0.21.0**, latest **0.20.0**, droits push suffisants, quota core **4 996 / 5 000** disponible au préflight. Tag/source immuables et worktrees conservés. GitHub Status indique tous les services opérationnels ; aucun incident correspondant dans l'historique officiel retourné. Cela ne prouve pas la cause interne de l'échec précédent.
- **Précision du diagnostic précédent :** le code exact de `softprops/action-gh-release@3bb1273…` appelle `generateReleaseNotes` dans `prepareReleaseMutation` **avant** le POST de création. Le catch commun affiche « Creating new GitHub release » / HTTP 500 pour les deux chemins. Les anciens journaux ne contiennent ni URL exacte de la requête échouée ni headers/request-id : **l'endpoint fautif (generate-notes ou create-release) n'est pas établi**. Permissions originales Contents/Attestations write ; payload explicite valide, notes préparées de **6 701 caractères**, bien sous la limite de 125 000. Ne pas présenter une panne temporaire ou un défaut reproductible comme prouvé.
- **Contrôles réels nouveaux :** ZIP récupéré égal au digest API **4620f342…**, dix fichiers/neuf SHA revérifiés ; **neuf attestations vérifiées cryptographiquement** avec certificat exact `release.yml@refs/tags/v0.21.0`, source/signer digest **3d74686…**, prédicat SLSA et refus des runners self-hosted. Authenticode réel des deux EXE ZIP et du MSI : **NotSigned**, aucun certificat signataire/horodatage. Les CLI macOS tar n'ont pas de commande Mach-O de signature ; les logs natifs des DMG disent ad-hoc seulement, sans Developer ID/notarisation/Gatekeeper. Aucun exécutable installé ou lancé sur l'hôte pour ces lectures.
- **Méthode prévue :** un seul POST officiel `gh api` crée un **brouillon**, avec notes complètes explicites, `generate_release_notes=false`, `make_latest=false`, tag existant et source exacte. Uploads séquentiels sans clobber/remplacement, vérification du brouillon complet, puis publication stable/latest seulement si tous les contrôles réussissent. Pas de rerun aveugle de l'action (`overwrite_files=true`).
- Preuves de cette reprise : **`%LOCALAPPDATA%\MicCamWatchQualification\0.21.0-publication-recovery-20261026`** ; admission API, état/historique officiel GitHub, diagnostic des deux chemins HTTP, bundle revérifié, neuf reçus de provenance, lectures Authenticode/CLI macOS, logs natifs et payload/notes explicites. Les deux agents site/documentation restent **sans édition** jusqu'à PACKAGE_GATE_PASSED. Préserver le contexte d'audit confidentialité 0.20.0 au futur changement de version, sans prétendre à une nouvelle revue juridique.

### Release GitHub effectivement publiée et vérifiée — ouverture du gate site

- **[v0.21.0 publique](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.21.0)** : ID **424498504**, published_at **2026-10-26T16:38:28Z**, non-draft/non-prerelease, endpoint latest au même ID. Tag officiel **06d800ba… → 3d74686…** inchangé. Aucun nouveau HTTP 500 ; aucun build relancé, aucun fichier publié remplacé. Le run historique **40331094727 reste failure**, jamais maquillé en success.
- Récupération officielle : création unique d'un brouillon avec notes explicites ; correction UTF-8 des notes de brouillon avant vérification publique ; après refus du lookup de `gh release upload` (« release not found », aucun upload), utilisation de son `upload_url` REST par ID. **Dix uploads séquentiels uniques**, chacun contrôlé contre taille/digest API. L'association `untagged-79cdef6f0f358410c515` exposée durant le staging a été explicitement corrigée vers le **tag déjà existant v0.21.0** : mêmes dix IDs/bytes, pas de recréation/déplacement du tag officiel. La ref auxiliaire générée par GitHub pointe aussi vers 3d74686 et n'a pas été supprimée silencieusement.
- **Gate sur les vrais fichiers publics passé :** dix téléchargements HTTPS **HTTP 200**, tailles/SHA égales à l'API et au bundle original ; neuf SHA du manifeste vérifiées ; neuf attestations cryptographiques **sur ces fichiers téléchargés** contraintes au certificat exact `release.yml@refs/tags/v0.21.0`, source/signer **3d74686**, SLSA, runner hébergé et invocation **40331094727**. Architectures/packages/SPDX/reçu ZIP inspectés ; manifeste lui-même non attesté.
- **Signatures réellement disponibles :** trois fichiers Windows Authenticode **NotSigned**, ni signataire ni horodatage ; tar CLI macOS sans `LC_CODE_SIGNATURE`, donc non signés ; logs natifs DMG ad-hoc seulement, sans Developer ID/notarisation/Gatekeeper garanti. ARM64 hébergé toujours distinct d'une validation Raspberry Pi/SBC physique. Pas d'installation ou de modification de protection sur le PC personnel.
- **Correction de présentation demandée pendant la reprise : titre affiché exactement `v0.21.0`.** PATCH limité au champ `name` ; notes, tag/ref/source, statut/date de publication et dix IDs/tailles/digests inchangés vérifiés. La convention future est déjà celle du tag (`config.input_name || tag`) dans l'action épinglée : **vX.Y.Z**, aucune modification du workflow nécessaire.
- PACKAGE_GATE_PASSED envoyé aux deux agents seulement après ces preuves. Lot site sept langues et lot README/guides autorisés ; parent intègre, teste et déploie ensuite. **Huit exports confidentialité 0.20.0 sauvegardés avant changement** ; leur contexte de revue doit rester gelé plutôt que s'attribuer automatiquement une nouvelle revue juridique. Preuves détaillées dans `…\0.21.0-publication-recovery-20261026`, notamment `public-package-gate.json`, `public-downloads.json`, `release-title-invariants.json` et `published-release.json`.

### Site/documentation intégrés — contrôles locaux avant déploiement

- Lot intégré : README **231 lignes**, huit guides/nav/historique/signatures/architecture, ce plan/ROADMAP et maintenance du site ; dix fichiers site pour version/pins, deux archives Linux et sept catalogues. Aucun changement application/native/installer/workflow. Le contexte `privacyRelease` conserve explicitement la revue **0.20.0/a265220…** ; maintenance documentée dans `docs/WEBSITE.md`, sans nouvelle revendication juridique.
- **Exécuté :** `node --test site/localization.test.mjs` **5/5 pass**, puis `node site/build.mjs` export réel **0.21.0**. Huit accueils contrôlés : canonical/huit hreflang/WebSite unique/MicCamWatch/URL de téléchargement 0.21, ARM64 et pin archive commit ; export `install.sh` SHA **ef5dc62e…**. **Huit pages confidentialité byte-identiques** à l'export précédent et **40 clés privacy inchangées dans chacun des sept catalogues**. Vérificateur des documents : **242 liens locaux/fragments**, zéro erreur. `git diff --check` passé.
- **Surface réellement observée :** ouverture de l'export dans Chrome isolé, onglet Linux activé normalement et panneau desktop anglais examiné avec les deux archives et avertissement ARM ; onglet français activé après défilement normal. Les tentatives de matrice mobile/JS on-off ont rencontré les timeouts CDP/`tab.observe` du navigateur géré et n'ont pas retourné de preuve complète : **ne pas déclarer 28 cas ou une validation visuelle mobile réussis**. Preuve partielle et limitation enregistrées, aucune fonction du site supprimée pour contourner ce problème.
- Prochain contrôle : workflow **Static website** sur le commit intégré uniquement, sans rejouer Release ni les cinq builds applicatifs ; puis avance rapide main/déploiement Git Pages autorisé après contrôle du diff et des refs. Vérifier les vrais HTTPS publics, confidentialité/SEO/pins après déploiement. Les avertissements signatures/ARM et la ref officielle restent inchangés.

### Contrôles site terminés — préparation du push de déploiement autorisé

- **[Static website 40364732189](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40364732189) success** sur **1ac2f237703f8fa0406a247b6bbda8b487803afc** : journaux lus, cinq tests passés, export réel 0.21 et artifact **12669372892** téléchargé/inspecté. Les 44 fichiers concordent en contenu avec l'export local ; différences de fins de ligne Windows/Linux et deux lignes vides dans exemples `<pre>` documentées, sans changement de commande ou d'asset binaire. Ce ne sont pas des différences de packages de la release.
- **Confidentialité servie avant déploiement :** les huit vrais HTTPS ont été sauvegardés. Leur seul ajout par rapport à l'export CI est le bloc **Cloudflare Pages Analytics préexistant**, injecté par l'hébergement ; ni ajout, ni désactivation, ni changement de configuration par nous. Hors ce bloc comparé séparément, HTML légal identique. Comparer les bytes complets avant/après production pour confirmer sa préservation.
- **Limitation navigateur contournée sans modifier le site :** nouveau Chrome headless isolé et actions clavier courtes. **14 cas mobiles réellement réussis** : sept langues × JavaScript activé/désactivé, viewport **390 px**, activation Linux au clavier avec JS, trois panneaux statiques visibles sans JS, deux archives Linux et absence de débordement horizontal. Captures réellement examinées : desktop anglais et panneau Linux mobile français ; pas de revendication visuelle exhaustive de tous les OS/langues ni des 28 anciens cas interrompus.
- Prochain push : avance rapide non forcée de **main depuis 3d74686…** vers les seuls changements site/documentation validés ; aucune modification application/native/installer/workflow et **tag v0.21.0 inchangé**. Commit final sans préfixe Cloudflare de skip ; trailer GitHub `skip-checks: true` réservé à ce compte rendu documentaire, pour ne pas reconstruire inutilement les cinq plateformes, le même arbre site ayant sa CI dédiée réussie. Vérifier ensuite le check Cloudflare et les vrais contenus HTTPS.

### Publication et production vérifiées — mission terminée

- Main avancé sans force **3d74686… → 97a754d2c2a6f18cf79ad5c76d2262310fb516bd**, uniquement site/documentation ; arbre site égal à **1ac2f237…**, testé par **[Static website 40364732189](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/40364732189)**. Check **Cloudflare Pages success**, déploiement **d0735478-db11-48d0-893a-9995e28d027a** par l'intégration Git existante. Aucune modification de configuration fournisseur, aucun rejeu de Release ou build applicatif.
- **[Site public](https://miccamwatch.pages.dev/) 0.21.0 vérifié sur 21 URL HTTP 200** : huit accueils avec téléchargements 0.21/source 3d74686/pins corrects et SEO canonical/hreflang/WebSite conservé ; **huit réponses confidentialité complètes byte-identiques avant/après déploiement**, bloc Analytics Cloudflare préexistant compris ; robots, sitemap, SVG et PNG 96 px, `install.sh` SHA **ef5dc62e71659261de0ad79b6b558e7d2446106d3adaed6019c27e651ce454bd**. CSS/JS publics égaux à l'export qualifié. Contacts `mailto:miccamwatch@gmail.com` inchangés.
- **Surface publique réelle aussi exercée :** Chrome isolé à **390 px**, onglet Linux français activé au clavier, archives x64/ARM64 0.21, commande HTTPS adaptée vers le `install.sh` du site et empreinte affichée correcte ; capture du panneau réellement examinée. Les 14 cas mobiles sept langues/JS on-off précédents restent un smoke de l'export identique, pas une nouvelle qualification matérielle ou visuelle exhaustive des interfaces natives.
- Release **[v0.21.0](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.21.0)** stable/latest et titre exact confirmés ; dix IDs/digests publics inchangés après correction du nom et cutover site. Tag **06d800ba… → 3d74686…** intact, neuf provenances contraintes au tag/commit vérifiées. Windows **NotSigned**, tar CLI macOS non signés / app DMG ad-hoc sans Developer ID/notarisation ; ARM64 matériel physique et autres configurations manuelles demeurent hors validation. Aucun succès inventé pour le run historique échoué.
- Preuves durables **`%LOCALAPPDATA%\MicCamWatchQualification\0.21.0-publication-recovery-20261026`** : `production-main-push.json`, `production-deployment-verified.json`, `production-url-checks.json`, `public-browser-smoke.json`, `site-mobile-browser*.json`, `qualified-site-export-verification.json`, reçus API/provenance/signatures et vrais téléchargements. Les captures/hashes conservent leur scope local/CI/public ; pas de secret d'authentification enregistré.
- Prochain point : version **0.21.0 publiée et disponible**, plus de blocage de publication actif. Conserver les limites documentées et les cinq exclusions historiques ; ne pas retoucher tag/packages ni prétendre à une validation ARM physique. Les comptes rendus finaux site/docs seuls peuvent être poussés avec **[CF-Pages-Skip]** et skip CI, sans redéployer des fichiers identiques ni reconstruire les plateformes.

## 2026-10-26 — Audit avant refonte : installations Windows, Wrangler et langages

**Périmètre : diagnostic non destructif.** Aucun exécutable MCW lancé, aucune installation/désinstallation, modification de PATH/autostart/préférence/protection, suppression, compilation, test produit, refonte ou publication. Seule cette documentation est modifiée sur autorisation explicite. Mesures de sources sur **main/candidat 9d626544…**, pas sur l'ancien worktree **work/linux-arm64-021/cafd98d** ; version publique **v0.21.0/3d74686…** conservée.

### Installations réellement présentes et points d'entrée

| Élément | État constaté sans lancer MCW |
| --- | --- |
| MSI utilisateur | **0.21.0**, enregistré sous `{86CDFED5-BA07-4C91-8BC7-8A856198DC33}`, éditeur Roman Cuisset, installation du 26/10 ; `%LOCALAPPDATA%\Programs\MicCamWatch` |
| Fichiers MSI | `mcw.exe` SHA **55753140f5f73743fe49399c3fa7a126234ea860029b7f71d673ce80a3af8fe8** ; `mcw-tray.exe` SHA **5a0253ea90a77bf7478f6a9f1826290f6a6055723cb12bdf4975b89bdfb1b6fe** ; égaux aux deux exécutables du ZIP public 0.21.0 déjà vérifié |
| Menu Démarrer | Un raccourci utilisateur `MicCamWatch.lnk`, cible MSI `mcw-tray.exe --open`, répertoire de travail MSI ; ressource d'icône Microsoft Installer présente |
| Tray actif | Un seul `mcw-tray.exe` observé, **PID 29540**, session 1, chemin MSI, parent **Explorer** ; donc **0.21.0**. Aucun autre processus MCW ni service MCW visible au snapshot |
| Ancienne installation Cargo | `%USERPROFILE%\.cargo\bin\mcw.exe` et `mcw-tray.exe` = **0.20.0**, prouvé par égalité SHA des deux membres avec le ZIP officiel 0.20.0 lu en mémoire |
| Provenance Cargo | `.crates.toml` et `.crates2.json` enregistrent encore l'installation d'origine **0.15.1** depuis un ancien répertoire source temporaire. Cette version de manifeste n'est **pas** la version actuelle des fichiers, remplacés depuis |
| Terminal/PATH | Dans le PATH utilisateur et celui du processus d'audit, `.cargo\bin` précède le répertoire MSI : `Get-Command mcw,mcw-tray -All` résout **0.20.0 avant 0.21.0**. Une fonction/alias d'un terminal déjà ouvert n'a pas été introspectée |
| Autostart | Valeur **HKCU Run / MicCamWatch** vers `"%USERPROFILE%\.cargo\bin\mcw-tray.exe"` ; StartupApproved premier octet **2**, donc entrée approuvée/activée. Le prochain login est encore configuré pour l'ancienne copie Cargo, pas le MSI |

Une seule entrée MicCamWatch de désinstallation MSI est visible dans les clés utilisateur/machine 32/64 bits inspectées. Aucun autre raccourci MCW découvert dans les menus/bureaux/startup inspectés, aucune tâche MCW visible. Audit sans élévation : il ne constitue pas un inventaire exhaustif des autres comptes ou de tout emplacement portable arbitraire. Les nombreux `target/**/mcw*.exe` sont des builds/tests, **pas des installations utilisateur**. Aucun MCW trouvé dans les emplacements Downloads/Desktop inspectés.

Les données à conserver sont `%APPDATA%\MicCamWatch` (réglages/verrous), `%LOCALAPPDATA%\MicCamWatch` (historique **1 309 676 octets** au snapshot, état caméra et dossier `microphone-protection` avec intention/verrous), ainsi que les éventuels journaux privilégiés de restauration. Aucun contenu d'historique ni secret n'a été exporté. Présence/absence d'un processus ne prouve pas que les contrôles physiques sont libérés.

### Consolidation proposée — attend l'autorisation

1. **Conserver le MSI officiel comme application quotidienne.** Conserver Cargo/Rustup et tous les autres outils Rust ; ne jamais supprimer `.cargo\bin` en bloc.
2. Après sauvegarde et contrôle des obligations de restauration, migrer l'autostart depuis l'exécutable Cargo propriétaire vers l'exécutable MSI exact. Ne pas écraser aveuglément une entrée étrangère ; ne pas effectuer de libération micro/caméra sans accord distinct.
3. Option recommandée : désinstaller **uniquement le paquet Cargo MicCamWatch** par son gestionnaire, puis vérifier la résolution dans un nouveau terminal. Cela élimine le doublon de commande tout en gardant Cargo utilisable. Alternative si ce paquet doit rester global : placer le MSI avant Cargo dans le PATH utilisateur, après autorisation, et réserver les chemins explicites aux essais.
4. Développement futur : nouveau worktree issu du main publié, builds `target` ou préfixe de développement dédié, jamais `cargo install --path .` dans le préfixe quotidien par défaut. Aucun worktree n'est créé ou réinitialisé pendant cet audit.
5. Pour des consultations CLI de développement approuvées, des racines **APPDATA/LOCALAPPDATA limitées à un processus enfant** peuvent séparer préférences/historique/intentions. **`--config` seul ne les isole pas.** Aucun `MCW_HOME`, `--data-dir` ou `--no-controller` n'existe dans le contrat inspecté.
6. **L'isolation de fichiers n'isole pas le matériel.** Les verrous matériels sont globaux ; le singleton tray Windows est fixe. Un second tray de développement parallèle au stable n'est pas une instance indépendante prise en charge. GUI/contrôles : VM jetable approuvée ou transfert explicite de propriétaire, sans périphériques personnels pour les tests destructifs.

Pièges constatés par inspection : `watch --no-kill` interdit la terminaison de processus mais peut reprendre une protection demandée ; ce n'est pas un lancement sans contrôleur. `mcw-tray.exe --version`/`--help` ne sont pas des requêtes de version dans le launcher actuel et peuvent démarrer le frontend ; aucun de ces appels n'a été exécuté. `mcw.exe --version` possède bien un chemin précoce de parsing. Références : `src/main.rs:46-200`, `src/bin/mcw-tray.rs:26-41`, `src/windows_control.rs:272-322,500-714`, `src/frontends/tray.rs:487-523`, `src/settings.rs:153-208`, `src/autostart.rs:272-309`, `docs/INSTALLATION.md:49-76,174-223`.

### Git, Warp et artefacts locaux

Les **six worktrees** ont zéro modification suivie avant cette mise à jour documentaire. Le worktree original **cafd98d** conserve les cinq groupes non suivis historiques : `.wrangler/`, `WATCHDOG.yml`, `debug_macos/`, `docs/next_projects.md`, `installer/tests/__pycache__/`. Le worktree confidentialité **35f15f9** possède aussi son propre `.wrangler/` non suivi ; ne pas le nettoyer par ricochet. Le stash historique `mcw-local-handoff-preserved-before-0.17.2-sync-20261013` conserve **180 ajouts/25 suppressions** dans `docs/NEXT_AI_LINUX_MACOS.md` ; ne pas appliquer/supprimer sans décision. `secrets.env` est ignoré/non suivi et n'a pas été lu.

Le compteur **+1689 de Warp n'est pas assimilé à une preuve de modifications de code** : Git montre zéro diff suivi dans le worktree concerné et 19 fichiers Wrangler non suivis. La sémantique exacte de cette vue Warp n'a pas été établie.

| Emplacement du worktree original | Fichiers | Octets logiques constatés | Décision proposée, non exécutée |
| --- | ---: | ---: | --- |
| `.wrangler/` | 19 | **3 807 818** | Exclure de Git ; sauvegarder/évaluer l'état local avant toute purge |
| `installer/tests/__pycache__/` | 3 | **97 483** | Bytecode généré, candidat à suppression approuvée et exclusion |
| `site-dist/` | 36 | **870 531** | Export généré déjà ignoré ; régénérable depuis sa branche/source |
| `target/` | 50 131 | **33 895 206 558** | Principal volume généré ; nettoyage ciblé approuvé possible, recompilation ensuite. Taille logique, pas une promesse d'espace physique récupéré |
| `debug_macos/` | 4 | **60 184** | Sources/notes de diagnostic historiques à conserver, pas un cache |

Également hors dépôt : neuf anciens `.cargo\bin\.mcw-update-*`, **77 323 313 octets**, et trois anciens fichiers MCW `.old`/`.bak`. Ce sont des sauvegardes de mises à jour, pas d'autres installations actives. Ne jamais les exécuter/restaurer comme contournement antivirus ; suppression éventuelle distincte après accord et contrôle du propriétaire. Ne pas supprimer des fichiers de verrou persistants parce qu'ils font zéro octet. `CARGO_TARGET_DIR` est absent des trois scopes et aucun override Cargo `config/config.toml` n'a été trouvé aux emplacements inspectés ; les autres worktrees restent néanmoins hors nettoyage.

**Wrangler/SQLite :** un cache JSON **2 042 octets**, six bases **24 576 octets**, six WAL **3 584 592 octets**, six SHM **196 608 octets**. Origine cohérente avec les qualifications Pages/Wrangler 4.149.0 documentées les 17–18 octobre (`ROADMAP` historique, `NEXT_AI_LINUX_MACOS`) ; la commande créatrice exacte n'est pas prouvée. `cache/cf.json` est une fixture de métadonnées de requête Cloudflare ; `state/v3/cache` est du Cache API persistant, distinct de ce cache de configuration.

Les six en-têtes SQLite et WAL ont été vérifiés. Lecture des seuls schémas de bases via URI **`mode=ro&immutable=1`**, sans rejouer les WAL ni créer de sidecar ; les snapshots de base n'exposent pas de table. **Cela ne prouve pas un état vide** : des données/diagnostics locaux D1/KV/R2/observabilité peuvent être dans les WAL. Aucun processus Wrangler/Miniflare/workerd visible au snapshot, mais aucun état n'est déclaré jetable sans sauvegarde. Après inspection, les 19 fichiers et leur volume restent identiques.

`.gitignore` actuel ignore `/target`, `/secrets.env`, `/site-dist/` et les exports temporaires, mais **pas Wrangler ni le bytecode Python**. Ajouts proposés, non appliqués : **`/.wrangler/`**, **`__pycache__/`**, **`*.pyc`**. Ne pas ignorer globalement `*.sqlite*`, `debug_macos`, notes ou WATCHDOG : cela masquerait potentiellement de vraies sources/données.

### Langages : mesure exacte, architecture et évolution

API GitHub `/languages` et blobs suivis de **main 9d62654** : **3 124 348 octets** éligibles au total. Les pourcentages mesurent des **octets de sources**, pas l'exécution, la taille des exécutables, la RAM ou le nombre de fonctionnalités.

| Langage | Octets comptés | Part calculée |
| --- | ---: | ---: |
| Rust | **1 916 784** | **61,35 %** |
| Python | **630 685** | **20,19 %** |
| PowerShell | **224 349** | **7,18 %** |
| Swift | **150 571** | **4,82 %** |
| Shell | **94 802** | **3,03 %** |
| HTML | **33 468** | **1,07 %** |
| JavaScript | **32 322** | **1,03 %** |
| CSS | **25 199** | **0,81 %** |
| C | **12 445** | **0,40 %** |
| Ruby | **3 723** | **0,12 %** |

Répartition par périmètre : `src/` **1 909 149 octets Rust** et `build.rs` **7 635** ; `native/` **137 374 Swift** ; `installer/tests/` **578 215 Python + 221 987 PowerShell + 10 009 C** ; `installer/macos/` **5 355 Python + 13 197 Swift + 2 436 C** ; `distribution/aur/` **20 232 Python + 2 732 Shell** ; `distribution/homebrew/` **26 883 Python + 3 723 Ruby** ; `distribution/winget/` **2 362 PowerShell** ; installateurs shell directs **92 070** ; site **33 468 HTML + 32 322 JavaScript + 25 199 CSS**.

**19 fichiers Python suivis = exactement les 630 685 octets de GitHub.** Seize tests/qualification **578 215 octets (91,68 % du Python)**, deux qualifications de distributions **47 115 (7,47 %)**, un packaging macOS **5 355 (0,85 %)**. Aucun des 19 n'est un processus/fichier du fonctionnement courant installé ; dix-huit sont appelés par workflows et le dernier est un helper importé.

| Fichier Python | Octets Git | Rôle / consommateur réel |
| --- | ---: | --- |
| `installer/tests/native_camera.py` | 22 541 | Qualification caméra Linux / CI ; réutilise fixtures d'installation |
| `installer/tests/native_install.py` | 74 118 | Installation/upgrade/rollback, fixtures partagées / Unix install et autres suites |
| `installer/tests/native_linux_desktop.py` | 26 268 | GTK/launcher installé, langues/thèmes / CI |
| `installer/tests/native_linux_tray_width.py` | 27 260 | Tray Linux, profils, géométrie et helpers réutilisés / CI |
| `installer/tests/native_mac_portable_update.py` | 29 721 | Bootstrap/mise à jour native macOS / CI et Unix install |
| `installer/tests/native_macos_app.py` | 44 837 | Vraie app DMG/Finder, lifecycle et données conservées / CI |
| `installer/tests/native_macos_menu.py` | 79 023 | AppKit/menu/interface, importantes sources Swift de test embarquées / CI et macos-stop |
| `installer/tests/native_macos_profiles.py` | 12 594 | Inspection/permissions/propriétaire et Cancel pré-UAC macOS / CI |
| `installer/tests/native_macos_stop.py` | 36 333 | Diagnostic historique et régression stop natif / workflow manuel macos-stop |
| `installer/tests/native_pipewire_limits.py` | 26 516 | Pannes/limites du collecteur et captures Linux / CI |
| `installer/tests/native_resources.py` | 63 941 | Mesure réelle CPU/RSS/latence et baselines partagées / CI et autres suites |
| `installer/tests/native_tui_controls.py` | 15 888 | PTY/ConPTY et contrôles TUI / CI et helpers importés |
| `installer/tests/native_uninstall.py` | 53 678 | Désinstallation/provenance/restauration sous admission jetable / workflows dédiés |
| `installer/tests/native_windows_protection.py` | 29 979 | Lifecycle propriétaires/IPC/protection Windows / CI |
| `installer/tests/windows_ordinary.py` | 22 734 | Ancienne route de qualification compte batch / encore référencée par native-uninstall |
| `installer/tests/windows_uninstall_lock.py` | 12 784 | Helper importé : contention réelle du worker de désinstallation |
| `installer/macos/package.py` | 5 355 | Packaging d'un Rust déjà compilé, app/icône/DMG ; non copié dans l'app |
| `distribution/aur/qualify-native.py` | 20 232 | Qualification makepkg/pacman / arch-package-native |
| `distribution/homebrew/native_qualification.py` | 26 883 | Qualification brew/LaunchAgent / workflow manuel homebrew-native |

Les cinq plus gros représentent **315 597 octets (50,04 % du Python)** : menu macOS, installation, ressources, désinstallation et app macOS. Pas de script complet démontré sans consommateur. Une suite manuelle ou une ancienne baseline n'est pas du code mort par définition.

| Tag inspecté | Rust propre, octets | Fichiers Python | Python, octets |
| --- | ---: | ---: | ---: |
| v0.2.0 | 22 473 | 0 | 0 |
| v0.10.0 | 178 326 | 0 | 0 |
| v0.15.1 | 869 621 | 1 | 41 700 |
| v0.16.0 | 961 797 | 2 | 80 634 |
| v0.17.2 | 1 371 461 | 11 | 304 829 |
| v0.19.1 | 1 530 364 | 14 | 391 524 |
| v0.20.0 | 1 635 669 | 18 | 534 421 |
| v0.21.0 / main actuel | 1 916 784 | 19 | 630 685 |

Il s'agit de reconstruction des **blobs Git**, pas de snapshots des anciens pourcentages Linguist. L'arbre v0.2.0 ne contient comme sources de programmation que Rust (hors workflows YAML). Introduction identifiée de la suite Python : **ef744eb**, 1er octobre, qualification de l'installateur Unix après v0.14.0. Le volume Rust n'a pas été remplacé : il a presque doublé depuis v0.16.0 ; la qualification multiplateforme a grandi plus vite.

**Frontière architecturale vérifiée :** exécutables `mcw`, `mcw-tray` et helper caméra en Rust ; Win32/GTK pilotés par Rust ; AppKit/helper macOS en Swift compilé et embarqué ; petit launcher Finder C. Les dépendances Python GI/PTY des tests ne sont pas des dépendances du frontend livré. Références : `Cargo.toml:11-24`, `src/lib.rs`, `src/frontends/tray/{windows,linux}_desktop.rs`, `build.rs:77-178`, `src/platform/macos.rs:32-43`, `installer/macos/package.py:23-110`, `installer/macos/launcher.c`.

**Exception importante, ne pas affirmer « aucun Python requis nulle part » :** `installer/install-camera-helper.sh:1-6` lance **`/usr/bin/python3 -I`**, avec du Python embarqué pour l'installation administrative Linux optionnelle (ELF, permissions, transactions/restauration). Cette dépendance est documentée dans INSTALLATION/AUR, distincte de l'exécution normale du helper Rust. Le cycle de vie Unix utilise aussi du shell ; Swift est du vrai runtime natif. L'audit ne démontre pas une fonctionnalité de surveillance migrée inutilement en Python. Une réécriture du bootstrap sensible exigerait une justification et une requalification de sécurité, pas un objectif cosmétique.

### Linguist, fichiers générés et améliorations proposées

Le Rust brut suivi fait **2 192 314 octets**, dont **275 530** de `vendor/libspa` correctement exclus ; Rust propre = chiffre API **1 916 784**. `.gitattributes` contient seulement **`installer/*.sh text eol=lf`**, aucun override Linguist. Les 19 `.py` sont des sources maintenues, pas leurs résultats générés. Le suivi Git courant ne contient ni `.wrangler`, target, node_modules, bytecode, SQLite, journaux de qualification, EXE/MSI/DMG/archives de release ou objets compilés. `Cargo.lock`, vendor patché, icônes, captures authentiques et leurs métadonnées, recettes et fixtures historiques sont intentionnels, pas des artefacts à jeter.

**Aucun ajustement Linguist justifié actuellement.** Ne pas classer les tests Python « generated », « vendored » ou les masquer en bloc pour remonter Rust ; `linguist-generated` les exclurait aussi des diffs. Ne pas réintégrer le vendor pour gonfler Rust. Les gros littéraux Swift sont comptés dans leur fichier Python ; le Python embarqué du bootstrap est compté comme Shell : les pourcentages ne sont pas une décomposition parfaite de l'architecture. Sources officielles : [GitHub languages](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-repository-languages), [Linguist overrides](https://github.com/github-linguist/linguist/blob/main/docs/overrides.md).

Lots futurs **à approuver, aucun correctif appliqué** :

- **P1 — Consolidation locale MSI/Cargo/autostart** suivant le plan ci-dessus, sauvegarde des données et conservation de la chaîne Rust. Aucun nettoyage ne doit toucher les enregistrements natifs de restauration.
- **P2 — Hygiène locale/Git** : exclusions ciblées Wrangler/bytecode, sauvegarde de tout l'état SQLite avec WAL/SHM avant décision ; suppression autorisée séparément de builds/exports/caches et anciennes sauvegardes de mises à jour. Préserver sources de diagnostic, notes, stash, autres worktrees et caches globaux.
- **P2 — Qualification Windows historique** : la route `windows_ordinary.py` encore appelée par `native-uninstall.yml` prépare une session batch/session 0, alors que l'admission actuelle exige WinSta0 interactif standard (`windows-fixture-context.ps1:188-193`, `native_uninstall.py:818-864`). **Incompatibilité déduite par inspection, pas réexécutée pendant cet audit.** Séparer proprement qualification headless et véritable route interactive plutôt que supprimer des contrôles ou répéter le scénario inadapté.
- **P2 — Provenance Homebrew** : `native_qualification.py` conserve une constante de défaut **0.17.2/a58c8b9…** alors que la formule consommée est **0.20.0/a265220…**. Faire dériver les métadonnées de la recette réellement consommée ; ne pas falsifier les preuves ni remplacer les baselines historiques par « latest ». Recettes Homebrew/AUR 0.20 restent distinctes des packages GitHub 0.21.
- **P3 — Simplification ciblée des harnesses** : partager davantage la préparation de session Linux déjà partiellement factorisée entre `native_linux_desktop` et `native_linux_tray_width`, sans fusionner leurs assertions spécifiques. Extraire si utile les sources Swift manuscrites de `native_macos_menu`/`native_macos_profiles` dans de vrais fichiers Swift de test pour lisibilité/outillage ; conserver assemblage, accès aux membres privés, entrée `main.swift`, fixtures et preuves. Cela redistribue Python/Swift, pas le volume architectural Rust.
- **P3 — Ergonomie d'audit du launcher** : envisager une vraie gestion précoce de `--version`/`--help` dans `mcw-tray`, sans ouvrir la fenêtre ni démarrer de surveillance ; comportement actuel documenté comme piège, non corrigé ici.
- **Règle d'architecture pour la future refonte** : logique de surveillance, modèle, permissions, IPC et propriété de protection restent en Rust ; assistance native Swift/C ciblée assumée ; Python cantonné à l'outillage adapté et au bootstrap administratif explicitement documenté. Pas de réécriture massive, minification, seuil artificiel « >50 % » ou baisse de couverture pour un graphique GitHub.

**Point de reprise : attendre l'accord du propriétaire sur les lots de consolidation/nettoyage et les éventuels correctifs d'outillage.** Seule cette ROADMAP est sauvegardée ; aucune autre source, `.gitignore`, `.gitattributes`, tâche, compte, PATH, base, installation ou protection n'a été modifié. Aucun push/main/tag/release/déploiement prévu par cet audit.

## 2026-10-26 — Maintenance Windows et hygiène Git effectuées

**Périmètre autorisé et respecté :** consolidation MSI/Cargo et exclusions Git ciblées. Aucun travail de design, aucune désinstallation MSI, aucun nettoyage de `target/`, des bases Wrangler, des worktrees ou du stash. Aucun tag, release, déploiement site ou push main.

### Consolidation Windows

- **État initial revérifié :** MSI **0.21.0** avec les deux exécutables aux SHA publics ; un seul tray actif, MSI, PID 29540, parent Explorer ; commandes résolues d'abord vers Cargo **0.20.0** ; Run `MicCamWatch` vers le tray Cargo et StartupApproved activé.
- **Protections contrôlées avant retrait :** intention microphone `requested:false`, `release_pending:false`, aucune restauration automatique ; caméra `blocked:[]`, `desired_blocked:false`, aucune garde active ; aucun fichier dans `ProgramData\MicCamWatch\CameraGuard` au snapshot. Aucune protection n'a été libérée ou modifiée.
- **Autostart :** la valeur Run existante a été repointée vers **`%LOCALAPPDATA%\Programs\MicCamWatch\mcw-tray.exe`**, en conservant son nom et StartupApproved. `mcw autostart status` retourne désormais **enabled**; une seule entrée Run, aucune tâche planifiée `MicCamWatch Tray`. Le prochain login n'a pas été simulé par déconnexion.
- **Retrait Cargo :** `cargo uninstall --root C:\Users\stcui\.cargo miccamwatch` a supprimé uniquement `mcw.exe` et `mcw-tray.exe` de `.cargo\bin` et l'entrée de registre Cargo. Aucun autre outil Rust n'a été touché. `cargo 1.98.1`, `rustc 1.98.1`, `rustup 1.29.1` restent opérationnels ; `cargo install --list` ne contient plus MicCamWatch.
- **Vérifications finales :** `Get-Command mcw,mcw-tray -All` ne retourne que le MSI ; `mcw --version` = **0.21.0** ; autostart **enabled** ; tray **running** ; raccourci `--open` testé, il réutilise le processus MSI unique. L'enregistrement MSI 0.21.0 reste intact.
- **Restes volontairement conservés :** anciens `mcw.old*.exe`, `mcw-tray.exe.v0.13.2.bak`, neuf `.mcw-update-*` (77,3 Mo) et `target/`. Ce ne sont pas des installations actives ; leur suppression reste une opération séparée.

### Hygiène Git

- Branche de maintenance **`maintenance/windows-git-cleanup-20261026`** créée depuis le commit d'audit **`b7550b0`**, qui était encore uniquement local sur `release/0.21.0-candidate`. Le diagnostic de Sol est donc conservé.
- `.gitignore` reçoit **`/.wrangler/`**, **`__pycache__/`** et **`*.pyc`**, sans doublon. Les mêmes motifs sont ajoutés au **`.git/info/exclude`** local partagé, afin que les worktrees existants cessent immédiatement d'afficher ces artefacts sans modifier leurs branches.
- Aucun fichier ou base n'est supprimé : 19 fichiers Wrangler (**3 807 818 octets**) et 3 `.pyc` (**97 483 octets**) restent sur disque. Aucun fichier suivi n'est ignoré et aucun artefact généré n'est suivi.
- État des worktrees : candidat propre hors `.gitignore`; original limité aux trois groupes historiques **WATCHDOG.yml**, **debug_macos/** et **docs/next_projects.md** ; confidentialité, GitHub, README et SEO propres ; stash et six worktrees préservés.
- `b7550b0` n'est pas poussé sur `release/0.21.0-candidate`; la branche de maintenance fournit la sauvegarde distante. `main` n'est pas modifiée et devra être mise à jour par une fusion explicitement autorisée.

### Risques et suites

- L'autostart est vérifié par registre et CLI, **pas par un redémarrage**. Le tray MSI déjà actif n'a pas été redémarré.
- Une future installation globale `cargo install --path .` pourrait recréer un doublon dans `.cargo\bin`; préférer les builds/worktrees de développement et les chemins explicites.
- Les anciennes sauvegardes Cargo, `target/`, les exports et les données Wrangler restent à nettoyer seulement après accord séparé. Ne pas supprimer les journaux de restauration ni les locks.
- La refonte graphique 0.21.1 n'est pas commencée ; les simplifications Python/Swift/CI documentées ci-dessus restent des travaux futurs.

## 2026-10-27 — Première intégration hybride réelle0.21.1, Windows en lecture seule

- **Décision du propriétaire : architecture hybride retenue.** Ne pas recommencer l'étude ni réécrire le moteur. Branche locale **`work/flutter-hybrid-0211`**, worktree **`%LOCALAPPDATA%\Temp\mcw-flutter-hybrid-0211`**, base `origin/main` **73369abda09bdadce8c779a9559b3e2334a32ecb** vérifiée. Flutter réutilisé depuis l'expérience **b8e1d7f/54e70f6**, sans fusion des prototypes ni anciens résultats simulés.
- **Tranche délimitée :** feature Rust opt-in `flutter-desktop`, DTO de lecture version1 borné/whitelisté, pipes anonymes privés hérités avec allowlist de handles, enfant suspendu contenu dans un Job Object avant exécution, fenêtre unique jetable et coalescence des seuls instantanés de présentation. Aucun message de mutation/protection/settings ni serveur réseau. Collecteurs/permissions/restauration/historique/CLI/tray/notifications restent Rust. Les fenêtres natives et leurs fonctions restent présentes, dont une entrée explicite de contrôles natifs pour le tray compilé avec la feature.
- **Qualification hôte prévue sans remplacer le MSI :** `mcw-tray --read-only-development` compile un vrai tray/PlatformMonitor dans un singleton de développement distinct, sans worker de contrôle, reprise de garde, automatisme de verrouillage/restauration, notification ou écriture de préférence/historique. Ce n'est pas une namespace opérationnelle alternative autorisée à modifier le matériel. États de protection consultés uniquement par les API passives existantes.
- Fichiers : `src/frontends/desktop.rs`, `src/frontends/desktop/windows.rs`, raccords `tray.rs` / `tray/windows_observer.rs`, feature/Cargo/mod et lanceur `mcw-tray`; `desktop/flutter/` reprend la présentation Flutter en remplaçant toutes les lignes fictives par le vrai protocole. État observé, couverture, confiance, application disponible, diagnostic et état demandé/réel/service restent distincts ; absence de donnée et panne ne deviennent pas « sûr ».
- **Vérifié à ce checkpoint :** build Rust de vérification passé après correction de l'appel Win32 `TrackPopupMenu`; analyse Flutter sans problème, **7tests de protocole passés**, build Windows release/AOT réussi. Clippy a relevé deux avertissements de style corrigés ; la revalidation Rust et la construction release sont en cours. **Pas encore de succès de bout en bout ou de chiffres de performance réelle revendiqué.** Avertissement MSBuild8029 lié au répertoire Temp conservé.
- [Contrat, sécurité et matrice de migration](FLUTTER_INTEGRATION.md) : réglages, commandes sensibles, historique et autres plateformes non migrés dans cette tranche ; interfaces natives sept langues préservées. Libellés Flutter de base en sept langues, textes techniques du moteur conservés ; pas de parité linguistique complète certifiée. Linux/macOS/runners Flutter non compilés à ce stade ; ARM physique/Intel et packaging futur restent à qualifier.
- **Point de reprise :** terminer builds/tests Rust, lancer le préfixe isolé à manifeste/SHA exact avec le mode de lecture, vérifier image réellement reçue + arrêt de Flutter + compteur d'observations avançant fenêtre fermée, répéter les cycles, profiler création/moteur/Dart/IPC/première image et mesurer mémoire. Le build Flutter release à rendu par défaut est conservé hors dépôt pour un avant/après honnête, sans moteur résident.
- Preuves et empreintes privées hors dépôt : **`%LOCALAPPDATA%\MicCamWatchQualification\flutter-hybrid-0211-20261027`**. Empreintes seules des préférences/historique/intentions, sans export de contenu privé ; inventaire du journal caméra protégé refusé par Windows, aucune élévation ou tentative de contournement. Aucun push/main/tag/release/site/installation personnelle autorisé ou effectué.

### 2026-10-27 — Checkpoint réel, profilage et qualification en cours

- Sauvegarde locale initiale **864bbb8**, aucun push. Rust Clippy avec `-D warnings`, les **14tests ciblés du bridge** et les deux binaires release ont réussi. Flutter : analyse, **7tests**, build Windows release/AOT réussis. Nouvelle passe Rust en cours après mise en forme des seuls nouveaux modules et ajout d'un refus sûr du mode de lecture quand la feature n'est pas compilée.
- **Première exécution réelle réussie :** fenêtre Flutter alimentée par `PlatformMonitor`, applications réellement observées, couverture/confiance et protections passives affichées séparément. Capture claire examinée, sans débordement au DPI125%. Aucun bouton de protection ou de réglage fictif.
- **Référence par défaut terminée :** trois essais indépendants, douze ouvertures/fermetures au total, un processus Rust fenêtre fermée, deux ouverte ; progression du compteur de collecte après fermeture et sortie de tous les PID de chaque essai. Le temps de création du moteur Flutter domine le démarrage ; les horloges enfant/parent sont distinguées.
- **Essai Skia Windows :** API officielle `DartProject::set_impeller_switch(Disabled)`, sans préchauffage ni moteur résident. Deux essais complets ont également passé activation répétée de la fenêtre visible, arrêt forcé du seul enfant graphique, collecte Rust continuée et réouverture, puis arrêt du parent avec enfant ouvert sans orphelin. Le troisième essai a été **rejeté**, pas validé : ouverture inattendue pendant l'intervalle de mesure « fermé ». L'origine de cette activation n'est pas établie ; ajout de marqueurs de source des demandes d'ouverture avant qualification finale. Conserver `skia-repeated/qualification.json`, y compris cet échec.
- **Défaut du harnais corrigé :** il cherchait le titre d'un HWND caché, effectivement vide ; identification désormais par classe + PID, avec nettoyage du seul processus épinglé en cas d'échec du constructeur. Le premier essai incorrect est conservé, pas présenté comme un défaut du tray.
- `desktop/qualification/windows.py` vérifie un manifeste/SHA du préfixe isolé, refuse le binaire stable connu, mesure mémoire privée/RSS/USS/CPU, capture uniquement le HWND possédé et vérifie les PID à l'arrêt. Les mesures ne comptent ni mémoire GPU ni DWM ; pas de conclusion sur l'absence de fuite à long terme.
- Logo Flutter générique remplacé dans le runner Windows par la copie strictement identique de l'icône officielle `installer/miccamwatch.ico` (neuf résolutions) ; aucun installateur modifié.
- **Prochain point :** terminer les vérifications Rust, reconstruire le runner final, répéter les mesures Skia avec traces d'activation, inspecter les captures sombre/diagnostics, vérifier les empreintes des fichiers privés et du MSI stable, puis consigner le bilan et les commandes reproductibles. Ne pas passer aux commandes sensibles/réglages/autres plateformes sans validation du propriétaire.

### 2026-10-27 — Tranche Windows réelle terminée, arrêt avant migration complète

- **Sources runtime sauvegardées et qualifiées : `743f5115dfa923404b3fbdd0427e86c1f28b0f27`.** Nouvelle passe Clippy `-D warnings`, 14tests Rust ciblés et builds release des deux binaires réussis. Chemin natif sans feature : Clippy et build réussis ; refus du flag de développement réellement observé avant tout lancement opérationnel, sortie2 après fermeture de son dialogue.
- **Qualification finale réelle passée :** trois essais indépendants, chacun avec quatre cycles ordinaires, activation répétée sans doublon, crash du seul enfant graphique, collecte continuée, réouverture et arrêt du parent avec enfant ouvert sans orphelin. Série sombre séparée passée ; captures claire/sombre et navigation Diagnostic réellement examinées au DPI125%. Le processus Rust revient à neuf threads après fermeture, contre onze avec le bridge ouvert.
- **Optimisation Windows conservée :** chemin Skia via l'API Flutter officielle, moteur toujours terminé à fermeture. Gain des médianes mesuré avant/après, mais un cas lent persiste dans la création du moteur. Chiffres, méthode, tailles, CPU, plages et limites dans [le bilan de première tranche](FLUTTER_0.21.1_FIRST_SLICE.md), pas de promesse « instantané ».
- **Échecs conservés et distingués :** mauvais identifiant HWND initial du harnais corrigé ; activation inattendue dans une série préliminaire restée sans cause établie, série exclue des comparatifs. Les trois essais finaux instrumentés ne montrent que les activations attendues. Le dossier initialement nommé `final-dark` est en réalité un essai clair ; seule la capture de `final-dark-verified` prouve le sombre. Compatibilité du manifeste UTF8 avec/sans BOM PowerShell vérifiée par la série finale.
- **Préservation mesurée :** exécutables MSI, préférences, intention micro et journal caméra utilisateur inchangés par SHA-256 ; tray stable d'origine toujours actif. Historique agrandi, mais tous les octets antérieurs conservés et vérifiés par le hash du préfixe initial. Pas d'écriture d'historique dans ce mode de développement. Journal ProgramData non lisible : ne pas prétendre l'avoir vérifié. Aucun UAC, changement de protection, installation ou désinstallation.
- **Reproduction et limites :** [contrat et commandes](FLUTTER_INTEGRATION.md), préfixe complet `final-runtime` et preuves sous `%LOCALAPPDATA%\MicCamWatchQualification\flutter-hybrid-0211-20261027`. Captures hors Git car elles contiennent des noms/PID d'applications réels. Le moteur est réel mais supplémentaire et en lecture seule, pas une connexion au MSI ; le raccord opérationnel n'est pas lancé ici.
- **À préserver pour la suite :** réglages/contrôles/historique et interfaces natives toujours présents ; pas de parité linguistique exhaustive certifiée. Windows10, autres DPI/GPU, Linuxx64/ARM64, macOS Intel/Apple Silicon et ARM physique non qualifiés par cette tranche. Packaging/mises à jour et sécurité des futures commandes restent des gates distinctes.
- **Point d'arrêt demandé :** présenter le bilan, les mesures et les captures au propriétaire ; **attendre son accord avant réglages, commandes sensibles ou autres plateformes**. Aucun push, fusion main, tag, release, déploiement ou remplacement du MSI. `main`/`origin/main` locaux restent sur73369ab ; tag v0.21.0 et huit worktrees conservés.

## 2026-10-27 — Flutter interactif, étape A en cours

- Validation du propriétaire : hybride conservé, interface commune visée, **livraison actuelle limitée à l'étape A**. Branche `work/flutter-hybrid-0211`, reprise propre surb5cf391 ; main73369ab, huit worktrees et stash conservés. Aucun push/main/tag/release/installateur autorisé.
- [Inventaire exhaustif CLI/tray/Win32/GTK/AppKit](FLUTTER_MIGRATION_MATRIX.md), sourcé surb5cf391 : prise en charge et limites natives, confirmations, réglages, profils, historique, notifications, maintenance et logos. Gaps à ne pas maquiller : pas de navigateur/export d'historique natif, show_ready persisté mais non consommé, pas d'alertes de capture/sons réellement câblés dans le tray Windows. Ne pas créer de nouveaux pouvoirs de blocage pour combler une case.
- [Contrat de commandes version2](FLUTTER_COMMANDS_STAGE_A.md) : une seule action typée `set_microphone_protection`, booléen explicite, défi CSPRNG par enfant transmis uniquement par pipe privé, ID monotone, génération/intention attendues, confirmation distincte mute/release, une opération en cours, résultat conservé avec état effectif séparé. Pas de commande shell/chemin/périphérique arbitraire ni privilège nouveau.
- Modules nouveaux préservés : admission portable `desktop/commands.rs`, client/panel Dart `lib/commands.dart`, sonde CPU par thread en lecture seule. Raccordement transport/DTO2 et worker natif en cours ; **ne pas supposer cette révision déjà compilée ni qualifiée**. Les sources de la tranche précédente et son préfixe exécutable restent disponibles.
- Accord explicite reçu via le choix **« Test local contrôlé »** : données de développement isolées, pas de garde active, toutes les entrées initialement rétablies avant un bref cycle SDK mute/release ; arrêter si état initial/restauration non prouvable. Aucun changement du MSI, de ses données ou d'une protection active, aucune restauration manuelle de vieux journaux d'ownership. Le release Windows rétablit toutes les entrées actives, **ne restaure pas chaque valeur d'origine**.
- CPU : inventaires processus répétés, signatures et moniteur sensoriel temporaire identifiés comme pistes, **pas encore de cause dominante mesurée**. Attente sensorielle500ms ne vaut pas500msCPU. La sonde de l'ancien PID28616 a constaté sa disparition et s'est arrêtée sans agir ; une mesure interrompue ne constitue pas un résultat. Ne pas diminuer les cadences ni introduire un cache de signatures par chemin/mtime.
- Point de reprise : finir le raccord du worker Windows et du mode de développement contrôlé, décodage/fraîcheur/UI, puis compiler/tester et exercer un cycle réel réversible uniquement après préflight. Conserver interfaces natives/autres plateformes. Captures, CPU/RAM/latences et résultats seront consignés avant l'arrêt pour validation du propriétaire.

### Étape A — raccord compilable et tests de contrat

- Raccord du protocole2 au superviseur, au worker microphone existant et à Flutter réalisé ; compilation Rust `cargo check --features windows-tray,flutter-desktop --all-targets` et analyse Flutter réussies. **24tests Rust ciblés puis27tests Flutter réussis** ; deux cas additionnels de confirmation du readback ont depuis été ajoutés et restent à exécuter. Les échecs initiaux Flutter étaient des timers de fixtures non libérés avant les invariants du binding, corrigés sans annuler les opérations natives à la fermeture d'un panel.
- Le nouveau mode explicite `--controlled-development` exige APPDATA/LOCALAPPDATA isolés sous le dossier de qualification et refuse un propriétaire matériel déjà actif. Le worker exige au premier geste un état entièrement rétabli/non demandé avant mutation. Le mode `--read-only-development` reste strictement sans commande ; aucun mécanisme caméra n'a été raccordé.
- Logo latéral Flutter remplacé par la copie identique du PNG officiel ; ICO Windows déjà identique à la source officielle. Assets et identités Flutter macOS/Linux encore non qualifiés/non unifiés ; pas de changement des installateurs.
- **CPU réellement mesuré sur60s, Flutter absent :47,79% d'un cœur**, dont47,71% sur un seul thread non principal ; le thread principal consomme0,026%. Sonde de chemins de collecte (5tours) : microphone≈31,25msCPU, caméra≈968,75msCPU, les deux≈1015,63msCPU en médiane. Les collecteurs restent sains dans cette série, sans changer la cadence de production. Le coût dominant est dans le chemin caméra ; signatures/modules et activité sensorielle restent à départager, pas de cache risqué ajouté.
- Prochain point : conserver ce checkpoint, finir l'attribution CPU, compiler le runtime final, exercer l'interface réelle et le cycle micro autorisé avec restauration contrôlée et traces. **Pas encore de réussite matérielle ni de capture interactive de l'étape A revendiquée.**

### Étape A — checkpoint de qualification avant rebuild graphique

- Sources sauvegardées jusqu'à `a8f23e7` (précédents points `6c5494c`, `b499c78`, `96a787a`). **26tests Rust de contrat/readback, Clippy `-D warnings`, builds release Rust/Flutter et check sans feature Flutter réussis** ; 27tests Flutter déjà réussis. Aucun push ni installation.
- Attribution CPU instrumentée temporairement :10collectes caméra, médiane registre/modules960,94msCPU ;20vérifications de signatures totalisent8531,25msCPU imbriquées dans9593,75msCPU registre/modules ; activité sensorielle médiane31,25msCPU/534,70msmur. Ne pas additionner les coûts imbriqués. Sondes temporaires retirées, source du collecteur restaurée à l'identique, aucune cadence/confiance modifiée ; le coût reste à optimiser séparément.
- Préflight matériel réel :1entrée, SDK unmuted, aucune protection demandée/garde active ; collecteurs sains, deux pipelines caméra Ready seulement. Des commandes reçues par la fenêtre ont effectivement produit des readbacks `Muted`/`Unmuted` avec succès. **Ne pas considérer les essais automatisés comme terminés** : plusieurs captures initiales blanches/échec PrintWindow, gestes concurrents non attribués, cycles interrompus avant confirmation automatisée complète.
- Rétablissement effectué par l'API native du préfixe isolé, confirmé à deux reprises sans interface ouverte ; aucun changement SDK spontané observé dans cet intervalle. Tous les processus de cette session contrôlée sont ensuite sortis normalement ; tray MSI9036 conservé. Journaux sous `%LOCALAPPDATA%\MicCamWatchQualification\flutter-commands-stage-a`.
- Capture Windows.Graphics.Capture mono-image, limitée au HWND/PID possédé, compilée hors dépôt avec le crate Windows0.62.2 déjà disponible ; aucune capture du bureau ni dépendance de production ajoutée. Une vraie capture montre les commandes actives, mais les premières images blanches doivent être rejetées, pas présentées comme validation.
- Défauts visuels constatés : dialogue excessivement large, corrigé par limite560pxlogiques et défilement ; icône Flutter toujours visible dans la barre de titre malgré un ICO source strictement identique à l'officiel. Prochaine étape : rebuild Flutter propre ciblé, vérifier la ressource réellement affichée, attendre une surface valide avant les gestes, terminer le cycle automatisé et les mesures. Ne pas migrer les autres commandes/plateformes.

### Étape A — cycle interactif Windows réellement réussi

- Runtime du cycle : Rust `a8f23e7`, UI reconstruite proprement sur `05d50ee`. SDK mute depuis Flutter confirmé (`requested=true`, `Muted`, garde active,1entrée), délai observé459ms ; fermeture réelle du processus Flutter, compteur d'observation430→435, seuls Rust/tray et sa garde native subsistent. Réouverture avec3demandes concurrentes : un seul enfant graphique, intention/garde conservées. Release distinct depuis Flutter confirmé (`requested=false`, `Unmuted`), délai observé411ms ; statut final garde arrêtée, SDK unmuted, caméra inchangée. Sortie normale de tous les PID possédés.
- Preuves durables : `final-interactive-controlled-cycle.json`, `final-confirmed-muted-light.png`, `final-confirmed-released-dark.png` dans le dossier de qualification. Ces captures de HWND/PID montrent les vrais états, le logo officiel en titre/latéral et le dialogue compact ; aucune donnée simulée. Le nettoyage/rebuild local Flutter a éliminé l'ancienne ressource titre bleue malgré l'égalité préalable des ICO sources.
- Captures : les images blanches et erreurs de capture antérieures sont conservées comme échecs d'outillage. La passe réussie maintient temporairement l'affichage requis pour son thread et restaure l'état précédent dans `finally`, sans modifier de réglage d'alimentation ni déplacer la souris globale. Ne pas confondre callback `ready`, rendu réellement capturable et état d'un écran inactif.
- Précisions de sécurité en cours de finalisation : retour de release nommé explicitement unmute (pas “mute confirmé”) ; libellé des sept langues restreint à la santé des **sources de surveillance**, car des collecteurs sains ne prouvent jamais toutes les captures possibles. Contrat/valeurs backend inchangés. Recompilation et mesures finales restent à faire après ces seuls textes.

## 2026-10-28 — Livraison Flutter A terminée, arrêt pour validation

- **Sources qualifiées `34e4e94c170565ef18482280241f9efb4a59a283`**, branche locale `work/flutter-hybrid-0211`. [Bilan détaillé, limites, procédures et reçus](FLUTTER_STAGE_A_RESULTS.md). Les checkpoints `6c5494c`, `b499c78`, `96a787a`, `a8f23e7`, `05d50ee` et `34e4e94` préservent les étapes ; ne pas repartir du prototype ni réattribuer les anciennes mesures.
- Reprise après interruption : aucun processus expérimental laissé ; le tray MSI a un nouveau PID2888, identifié par son vrai chemin, jamais par l'ancien PID9036. La dernière mesure interrompue n'était pas complète et n'est pas comptée. Aucun nouveau cycle matériel n'a été exécuté le28octobre.
- **Validation finale :26tests Rust ciblés,27tests Flutter, analyse Flutter, Clippy `-D warnings`, builds release réussis.** Le refus réel d'un mode contrôlé hors périmètre isolé est confirmé, sortie2. Les erreurs de contrat/readback et de confirmation sont testées ; pas de conflit matériel ou refus UAC artificiellement créé.
- **Mesures durables sur9ouvertures**, mode lecture seule, build34e4e94 : un enfant ouvert, zéro après chaque fermeture ; Rust/tray seul reste vivant. Premier protocole-ready944ms (un lancement), médiane des8réouvertures239ms (220–254), médiane de sortie du processus239ms. **Pas une mesure de premier raster réellement affiché** ; caches Windows non purgés.
- RSS médiane Rust seul21,51Mio avant, Rust+Flutter133,55Mio ouvert, Rust22,54Mio après9fermetures ; commit privé respectivement3,92/117,26/4,21Mio. Somme RSS peut doubler des pages partagées, GPU/DWM non inclus. Passe distincte du worker contrôlé fermé, sans ordre :19,28MioRSS/4,00Mio commit, un processus. Payload Flutter25,34Mio ; CLI+tray+Flutter37,47Mio,15fichiers SHA-256 vérifiés.
- CPU de cette passe≈10% d'un cœur (worker contrôlé fermé≈6,96%), mais **pas de correctif CPU revendiqué** : un seul pipeline caméra Ready aujourd'hui contre deux au diagnostic47,79%. Collecteurs, fréquence et confiance inchangés ; coût des signatures toujours à optimiser avec invalidations sûres, sans ignorer Ready ni cacher la confiance par chemin/mtime.
- **Captures finales34e4e94 réellement affichées/examinées**, français125%/DPI120, clair et sombre : `qualified-controls-light.png`, `qualified-controls-dark.png`, reçu `qualified-controls-captures-20261028.json`. Contrôles réels activables, données natives, libellé « Sources de surveillance », logo officiel titre/latéral ; zéro commande matérielle durant cette passe. Le cycle réel SDK antérieur, avec fermeture/garde/réouverture/release, reste conservé avec ses révisions exactes.
- Préservation finale : binaires MSI0.21.0 et6fichiers stables hors historique comparés identiques ; historique accru mais préfixe initial intact. SDK passif unmuted, demande/garde absentes ; seul le tray installé reste actif. Pas de modification caméra, permissions, autostart, PATH, installateur, site, branche publique ni autre worktree.
- **Point d'arrêt obligatoire : attendre l'avis du propriétaire sur cette étape A.** Caméra, préférences, historique, maintenance, autres plateformes et packaging Flutter restent hors livraison ; interfaces natives conservées. Sept langues fournies mais pas toutes validées visuellement ; lecteur d'écran/clavier complets et100%/150% DPI à qualifier. Aucun push/main/tag/release/déploiement autorisé. Prochaine décision : traiter le coût CPU restant et choisir le lot de parité B, uniquement après accord.

## 2026-10-28 — Après validation A : tranche B1 préférences en cours

- Reprise propre sur `80d43c9`, code A `34e4e94` ; main/origin73369ab, tag et8worktrees/stash préservés. Le propriétaire valide A et autorise une tranche fonctionnelle suivante, **sans push ni test matériel modifiant micro/caméra**.
- Périmètre B1 retenu : préférences natives notifications, sons, historique et profil d'évaluation ; sauvegarde conditionnelle sous le verrou stable existant, fusion des champs non concernés, valeurs enregistrées distinctes du profil réellement appliqué. Windows ne diffuse pas encore les alertes de capture/sons dans son tray : l'interface doit l'expliquer, sans transformer une préférence persistée en capacité opérationnelle.
- Checkpoint WIP : nouveaux `desktop/preferences.rs`, `preferences_model.dart`, `preferences.dart` et tests, plus `compare_cpu.py`. Admission de l'enveloppe factorisée avec la voie micro ; compteurs/opération en cours séparés par voie, même défi privé possédé. `Settings::update_checked` permet le refus sans remplacer le fichier. Worker de persistance borné, indépendant de Flutter et non bloquant pour la boucle du tray. **Pas encore compilé ni raccordé en bout en bout.**
- Protocole3 en cours de cutover propre : champ de préférences et événement typé supplémentaires, micro conservé ; aucun interpréteur de commandes arbitraires ou nouvelle autorité matérielle. Restent à raccorder superviseur, tray/observateur, mode de test settings-only isolé, DTO/fixtures et navigation Flutter. Ce mode ne doit créer aucun worker microphone ni reprise de protection.
- CPU : audit confirme les limites d'un cache de verdicts de signature, même local à un snapshot ; un handle de fichier ne fige pas l'état de confiance Windows. Plus petit gain sûr proposé : ne pas charger l'inventaire processus sensoriel lorsque tous les rapports collectés sont vides, sans déplacer l'ordre d'identité des rapports non vides. Mesures AB/BA avec charge vérifiée identique à produire ; aucune baisse CPU revendiquée.
- Préparation Unix : producteur/transport Flutter actuellement Windows-only, interfaces natives conservées ; profils et libération micro Unix ne doivent pas hériter de la sémantique Windows « tout rétablir ». Les workflows actuels qualifient les interfaces natives, pas Flutter. Plan de transport/authentification/packaging à documenter, sans port fictif ni validation ARM physique annoncée.
- Prochain point : finir le raccord, vérifier les nouveaux modèles/contrats, tester CAS/erreurs/timeout/persistance dans les seules données isolées, puis compiler, exercer clair/sombre et mesurer à conditions comparables. Sources A/exécutables qualifiés conservés ; ne pas réexécuter leur cycle matériel.

### B1 — raccord compilé, qualification réelle à poursuivre

- Point de sauvegarde initial `65b5ae8`. Protocole3 raccordé Rust/Dart/C++ ; quatre préférences typées, CAS sous verrou stable, travail de persistance possédé par le tray et non par Flutter. Profil enregistré et profil réellement appliqué distincts. Mode `--settings-development` limité aux chemins de qualification, sans worker microphone ni reprise de protections.
- Résultats de ce checkpoint : check/Clippy Rust avec `flutter-desktop`,42tests IPC et4tests de persistance réussis ; **ce flag seul ne compile pas le binaire tray, qui exige `windows-tray` : la qualification de celui-ci n'était pas établie ici**. Analyse Flutter et47tests réussis ; avertissement de cible de clic corrigé, puis20tests de préférences repassés sans avertissement. Build Flutter release réussi ; avertissement MSBuild MSB8029 lié au worktree sous Temp.
- Petit gain CPU implémenté : inventaire processus sensoriel évité uniquement lorsque tous les rapports de la collecte courante sont vides. Ni cache de signature, ni cadence modifiée. **Pas encore de mesure comparative ni de résolution du coût CPU revendiquée.**
- Prochain point : préfixes exécutables/manifeste SHA-256 isolés, vraie interface préférences clair/sombre, persistance/réouverture/attente du verrou, mesures AB/BA et préservation de l'installation stable. Aucun test matériel, push ou publication exécuté.

## 2026-10-28 — Tranche B1 livrée : préférences réelles, limites CPU conservées

- **Runtime qualifié `a0411ea80fd985c9aa319061c4ad3a1830880aeb`**, branche locale `work/flutter-hybrid-0211`. Checkpoints `65b5ae8` (WIP), `3a661e8` (intégration protocole3/préférences), `a0411ea` (inventaire sensoriel vide évité). [Contrat, résultats, commandes et préparation Unix](FLUTTER_PREFERENCES_B1.md) ; [matrice actualisée](FLUTTER_MIGRATION_MATRIX.md).
- Quatre commandes de préférences depuis la **vraie fenêtre Flutter** : notifications, sons, futur enregistrement d'historique, profil d'évaluation. Sauvegarde native conditionnelle/fusionnée et relue ; profil enregistré distinct de celui réellement appliqué au collecteur. Profil Private puis Balanced réellement constatés dans les deux champs. Aucun nouveau pouvoir de blocage, aucune modification du sens de la libération micro.
- Tests réels dans `--settings-development`, données dédiées sous `%LOCALAPPDATA%\MicCamWatchQualification\flutter-preferences-b1-20261028-142703` : refus d'écriture sur le seul fichier de test en lecture seule, octets/valeurs préservés et erreur UI explicite ; verrou stable maintenu, timeout UI incertain sans sauvegarde optimiste, collecte continuée ; fermeture/remplacement de Flutter sans annuler l'écriture, nouvelle session correctement bloquée jusqu'au résultat, réussite après déverrouillage. Toutes les préférences isolées initiales rétablies par Flutter, champs étrangers inchangés.
- Gestes fiables via MSAA sur HWND/PID possédés, sans déplacer la souris globale. Captures réelles française125% : `preferences-restored-light.png`, `preferences-confirmed-dark.png`, examinées. Logos titre/latéral officiels. Sept langues couvertes par les widgets de réglages, **pas sept validations visuelles natives** ; lecteur d'écran/clavier complets et autres DPI restent à qualifier.
- **Builds corrigés explicitement `windows-tray,flutter-desktop`** : check tous targets, Clippy `-D warnings`, build release des deux exécutables réussis. Variante native `windows-tray` seule : Clippy strict réussi. Dernière passe avec les deux features :42tests Rust desktop +4tests settings réussis. Flutter : analyse,47tests, build release réussis ;20tests préférences repassés après correction du geste de fixture. Aucun assouplissement de sécurité.
- Échec d'outillage conservé : le premier packaging avait repris l'ancien tray A car `flutter-desktop` seul faisait ignorer sa cible Cargo. Pas de HWND/tick attendu ; préfixes exclus, manifests archivés, anciennes copies créées ici retirées après succès final (≈76,23Mio), bundle A et runtimes v3 conservés. Ne jamais préparer un manifeste à partir d'un binaire `target/` dont le build a ignoré la cible. Les premiers gestes synthétiques non fiables et une initialisation MSAA trop précoce ne sont pas comptés comme réussites ; leurs reprises fiables sont distinguées.
- **CPU AB/BA apparié réellement réussi**, A34e4e94 contre B1a0411ea, deux tours,20s par intervalle fermé/ouvert, états CLI identiques aux bornes : un pipeline caméra Ready faible confiance, aucun micro actif observé, mêmes identités et collecteurs sains. Fermé :16,73→16,21% d'un cœur ; ouvert :19,04→16,37%. Deltas fermés +0,46 puis−1,50point : petit échantillon, **pas de gain CPU significatif ni résolution du coût dominant revendiqués**. Signatures toujours vérifiées, pas de cache par chemin/mtime, ordre d'identité/cadence conservés. Compteurs détaillés d'opérations non mesurés sur cette passe, anciennes sondes de signatures conservées séparément. Captures actives contrôlées, plusieurs applis et latence matérielle non testées, faute d'autorisation spécifique.
- RAM du comparatif : RSS fermé20,73→20,66Mio ; ouvert131,05→135,25Mio. Commit privé fermé4,25→4,11Mio ; ouvert116,83→119,76Mio. Mémoire partagée/GPU non assimilés au commit. Voir le bilan pour USS et méthode.
- Série B1 supplémentaire :6ouvertures/fermetures, un essai à3demandes concurrentes, un enfant ouvert et zéro après chaque fermeture. Premier protocole-ready541ms ;5réouvertures médiane562ms (504–619), sortie processus médiane423ms. **Pas une mesure du premier raster, ni un gain de lancement démontré sur A.** RSS avant/ouvert/après19,53/131,73/20,85Mio ; une sonde distincte après vraies écritures inclut le worker persistant :20,27MioRSS,5,12Mio commit privé, un processus. CPU19,26% de cette sonde non appariée non attribué artificiellement au worker. Bundle15fichiers38,22Mio, dont Flutter25,88Mio.
- Préservation vérifiée :9fichiers accessibles de l'installation/données stables, binaires et hors historique identiques ; préfixe historique original conservé. Tray MSI2888 identifié par son vrai chemin toujours actif ; aucun processus de test restant. Aucun UAC, autostart/PATH, installateur, commande matérielle, suppression d'historique stable, push/main/tag/release/site.
- Unix : plan de transport possédé, cycle de vie, restauration spécifique, logos/identités, sandbox/macOS15 et packaging documenté. Aucun port Flutter Unix ni nouvelle CI distante exécuté ; GTK/AppKit conservés. Aucune validation ARM physique revendiquée. Le raccord opérationnel Windows est compilé mais pas lancé sur le MSI du propriétaire ; la preuve réelle porte sur les préférences isolées.
- **Point d'arrêt demandé : présenter B1 et attendre validation avant migration plus large/publication.** Priorités proposées : coût des signatures restant, pause/reprise des notifications et inspection/autostart à qualifier en isolation, détails devices/doctor ; caméra/lock et Unix après leurs gates propres. Ne pas inventer de navigateur/export historique ou de préférence langue/thème persistée absents du moteur.

## 2026-10-29 — Tranche B2 en cours, checkpoint d’intégration

- Reprise propre sur `4655f96`, main/origin73369ab, tag v0.21.0 inchangé,8worktrees et stash conservés. Le propriétaire valide B1 ; aucun push, installation stable ou test matériel autorisé.
- Alertes Windows raccordées aux snapshots existants via politique de transitions/cooldown et worker natif borné ; master/pause/audio persistés relus au moment de la livraison. Identité `MicCamWatch.Qualification` distincte du MSI pour les seuls essais explicites TEST. Les rapports Submitted signifient acceptation WinRT, pas visibilité/audibilité prouvée.
- Nouveau superviseur Flutter Unix et raccord du tray partagé Linux/macOS, IPC privé et préférences typées, matériel désactivé sur cette première voie ; modes de développement stricts avant initialisation opérationnelle. DTO créé seulement pour une fenêtre ouverte ; dernière observation native déplacée/conservée pour réouverture fraîche. GTK/AppKit conservés en fallback. **Unix non encore compilé ni exécuté à ce checkpoint.**
- Runners Unix : logos officiels, produit visible MicCamWatch, activation possédée ; bundle macOS `desktop/MicCamWatch.app/Contents/MacOS/MicCamWatch`, Linux `desktop/miccamwatch_desktop` avec runtime complet. Entitlements/signature natifs inchangés. Aucun macOS accessible : code préparé, aucune compilation/validation Mac revendiquée.
- Windows : check, Clippy strict,10tests notifications,42tests desktop et build release avec `windows-tray,flutter-desktop` réussis. Deux défauts Clippy initiaux corrigés sans assouplir la règle. Les messages de capacités Flutter sont maintenant neutres vis-à-vis de l’OS ; analyse/build graphique à refaire.
- Profilage local opt-in ajouté (`MCW_COLLECTOR_PROFILE=1`, feature expérimentale) : scopes imbriqués snapshot, inventaire, microphone, caméra, signatures/WinVerifyTrust/signataire ; aucun verdict mis en cache ni cadence réduite. Première sonde API indépendante archivée, **pas encore de gain moteur revendiqué**. Prochain point CPU : profiler le vrai observer puis choisir une optimisation justifiée.
- Linux disponible : WSL2 Kali x64, noyau6.6.87, WSLg X11/Wayland ; pas de SDK/dev libs initialement. Préparation strictement privée sans `apt install`, hooks système ou élévation : extraction des paquets vérifiés, SDK officiels à SHA-256. Un téléchargement fondé sur les index APT existants a reçu404 ; index privés authentifiés en préparation, aucun package système modifié.
- Reprise : qualifier d’abord le mode Windows TEST sans Flutter, persistance/audio/erreurs et identité stable ; profiler CPU réel à charge appariée. Finir les outils Linux privés, compiler/corriger puis afficher/exercer le vrai frontend WSLg. Préserver les preuves B1, ne pas lancer le tray opérationnel sur les données stables, documenter limites Mac/ARM et publier uniquement un bilan B2 après qualification.

### B2 — qualification locale et corrections de revue en cours

- Point de sauvegarde précédent `27948eb`. Windows et Linux : analyse Flutter sans erreur, **47tests Flutter réussis sur chaque OS**. Build Flutter Windows release réussi ; MSB8029 lié au worktree Temp reste un avertissement d’outillage. Premier `cargo check --all-targets --features flutter-desktop` Linux réussi avant les corrections de revue ci-dessous ; avertissements préexistants des dépendances vendoriées conservés.
- Outils Linux privés prêts sous `/home/roman/mcw-qualification/b2-20261029-082656` : Rust1.98.1 et Flutter3.47.8 officiels vérifiés SHA-256, paquets issus des index APT privés authentifiés, extraction seule. Aucun paquet/hook/configuration système installé. Build graphique Linux : dépendance MIME déjà installée réutilisée en lecture seule ; trois bibliothèques runtime absentes du sysroot privé ajoutées avec vérification SHA-256. Édition de liens encore à terminer, aucune interface Linux affichée à ce point.
- Profil Windows réel, Flutter fermé,30s :28,92% d’un cœur,22,28MioRSS,42cycles et84vérifications WinVerifyTrust. Scopes **inclusifs, non additionnables** :4,578sCPU WinVerifyTrust,1,734s inventaire processus,8,781s snapshot complet. Cette charge diffère de B1 : pas de comparaison directe ni de gain revendiqué.
- Optimisation ciblée ajoutée : snapshot frais `SystemBasicProcessInformation` sur Windows11 compatible, génération noyau unique pour réutiliser uniquement la date de naissance immuable d’un processus. Toute nouvelle date est confirmée par une seconde génération courante ; les arrivées/réutilisations non échantillonnées imposent le fallback Toolhelp. Aucun verdict signature/token/politique, chemin ou module en cache, aucune cadence réduite. Quatre tests initiaux réussis ; garde d’ascendance supplémentaire ajoutée après revue, tests/Clippy/build et mesures appariées à refaire avant qualification.
- Revue Unix : six défauts démontrés traités dans le code (groupe possédé nettoyé avant reap, payload exécutable validé, ancêtres XDG épinglés, observateur stop-and-join, fallback natif sur échec avant ready, décès du parent indépendant de Dart). Correctifs et tests ajoutés par les agents, **pas encore compilés/exécutés**. macOS reste sans machine de compilation ; contrôle natif du parent préparé, aucune validation Mac revendiquée.
- Premier essai de notifications TEST réel : Flutter absent, un seul processus, sortie propre ;2échecs WinRT `0x80070490`, donc **aucune remise réussie revendiquée sur cet essai**. Cause isolée par sonde native : pour un AUMID neuf, `Setting()` échoue avant le premier `Show`, puis le premier envoi officiel initialise le canal et permet sa relecture activée. Un toast TEST silencieux retrouvé dans l’historique du seul canal de qualification. Canal MSI lu uniquement pour disponibilité, jamais utilisé pour envoyer un test ; aucun raccourci/autorisation système modifié.
- Correctif du premier envoi ajouté : seul le catalogue absent autorise la tentative officielle ; les autres erreurs/refus restent bloquants, et une disponibilité activée est exigée après cette initialisation. Pas encore de build/essai du worker corrigé à ce point. Prochaines preuves : notifications/pause/master/audio persistés et Flutter fermé, mesures CPU AB/BA, vrai affichage/IPC/cycle de vie Linux, puis matrice et bilan B2. Aucun push, merge, tag, déploiement, installation stable ou action matérielle.
- Après corrections : Clippy Windows strict,5tests d’inventaire/générations,10tests notifications et build release des deux exécutables réussis ; **15tests du superviseur Unix réellement exécutés et réussis sur Linux**. Build Flutter Linux maintenant lié, mais installation du bundle refusée vers `/usr/local` : le cache CMake du premier configure incomplet a conservé son préfixe par défaut. Aucun droit élevé demandé ; remettre explicitement le préfixe sur le bundle privé avant la reprise, sans installation système.

## 2026-10-29 — Tranche B2 livrée, arrêt pour validation avant B3

- Branche locale `work/flutter-hybrid-0211` ; checkpoints `27948eb`, `d37c8e8`, puis `9aa4689`.
  [Bilan B2 détaillé et limites](FLUTTER_B2_QUALIFICATION.md) ; [matrice de parité](FLUTTER_MIGRATION_MATRIX.md).
  Aucun push, merge, tag, release, déploiement ni remplacement du MSI stable.
- **Notifications Windows effectives dans Rust**, indépendamment de Flutter : mêmes snapshots de capture, transitions Active seulement,
  cooldown30s, file bornée32, master/pause/audio persistés et relus avant livraison. Ready, répétitions et source incomplète ne créent pas de départ.
  Erreurs et file indisponible restent visibles ; Submitted n'est pas une preuve de bannière affichée ou de son physiquement entendu.
- Qualification isolée réelle : deux départs TEST remis Flutter fermé ; répétition immédiate supprimée ;
  désactivation par la vraie fenêtre Flutter, persistance après redémarrage, réactivation et son sauvegardés/relus.
  Pause/reprise par la CLI native vérifiées après redémarrage, Flutter fermé et collecte continuée.
  Historique WinRT du seul canal TEST : sept entrées, dont deux demandes audio non silencieuses et cinq silencieuses.
  Préférences privées remises à leurs valeurs initiales ; aucune commande matérielle B2.
- Premier AUMID neuf : le préflight `Setting()` pouvait échouer avant le premier `Show`.
  Correctif qualifié : seule l'absence du catalogue autorise la tentative officielle ; refus/politiques OS restent applicables,
  et disponibilité relue après initialisation. Les messages opérationnels génériques gardent leur comportement B1.
- **CPU Windows apparié**, builds release `27948eb` contre `d37c8e8`, deux tours AB/BA,20s par intervalle, mêmes états observés aux bornes :
  Flutter fermé17,53→11,60% d'un cœur ; ouvert15,94→15,07%, mais deltas ouverts de signes opposés, donc gain ouvert non établi.
  Une seule caméra Ready dans cette charge ; aucune latence de capture active ni plusieurs applis actives contrôlées revendiquées.
- Optimisation : inventaire frais `SystemBasicProcessInformation` sur Windows11 compatible ; cache des seules dates de naissance
  par PID/génération noyau, nouvelles naissances confirmées et fallback Toolhelp sur générations finales non échantillonnées.
  Aucun cache de signature, jeton, chemin, module, verdict ou autorisation. Windows10 garde le chemin ancien, non mesuré ici.
  Passe instrumentée séparée : inventaire12,87/14,91msCPU→0,45/0,22ms ; une vérification WinVerifyTrust par snapshot avant/après,
  34/34 puis36/36 snapshots/vérifications par intervalle et cadence non réduite. Les signatures restent un coût important.
- Mémoire Windows du comparatif : RSS fermé19,29→20,63Mio ; ouvert132,88→133,99Mio.
  Sondes fonctionnelles distinctes : worker après remise TEST fermé26,86Mio ; réglages ouverts143,23Mio ; même owner après fermeture20,12Mio.
  Ne pas mélanger ces modes en un seul benchmark. Bundle de qualification final Windows38,31Mio,15fichiers.
- **Linux x64 réellement exécuté sous WSLg/X11** : même Flutter release, snapshots Linux réels dégradés/inconnus,
  six ouvertures/fermetures sur les deux thèmes, demandes répétées réutilisant le même enfant, zéro Flutter après fermeture.
  Première fenêtre X11 visible≈2,08s puis0,87–0,97s : mesure de fenêtre mappée, pas de premier raster.
  Arrêt natif propre ; décès du parent jetable termine aussi un enfant explicitement suspendu via `PR_SET_PDEATHSIG`.
  Captures Windows/français125% et Linux/anglais100% réellement examinées ; pas sept validations visuelles natives.
- **Limite graphique Linux non masquée** : après stabilisation, llvmpipe logiciel confirmé,
  fenêtre ouverte270,30–271,24MioRSS et39,09% d'un cœur ; fermée5,75Mio et1,20%, un processus, aucune bibliothèque Flutter dans Rust.
  Ce n'est ni une validation Linux accéléré/Mint/Wayland physique ni une mesure du tray SNI opérationnel.
  Aucune cadence, permission, règle ptrace, configuration GPU ou protection modifiée pour améliorer les chiffres.
- Superviseur Unix renforcé après revue : groupe nettoyé avant reap, payload/ancêtres épinglés,
  observateur stop-and-join, fallback natif sur échec avant Ready, parent surveillé indépendamment de Dart.
  GTK/AppKit conservés. Sous macOS, monitor natif préparé mais suspendable avec tout le processus :
  pas de garantie noyau équivalente à Linux. Aucun hôte/Xcode accessible, donc **aucun build/runtime/IPC/visuel/package macOS qualifié**.
  Minimum effectif moteur/helper macOS15 conservé. ARM64 : préparation générique, aucune nouvelle compilation Flutter ni validation matérielle B2.
- Vérifications finales : Clippy Windows strict natif seul et hybride,5tests générations,10tests notifications,42tests desktop,
  builds release CLI/tray réussis ; Linux Clippy strict,49tests desktop dont15de confinement, build natif release réussi.
  Flutter Windows/Linux : analyse sans erreur,47tests chacun, builds release réussis.
  Cache CMake partiel corrigé vers un bundle privé ; garde `/usr/local` refusée par essai négatif avant installation, rebuild privé réussi.
  Aucun paquet système installé : SDK/paquets vérifiés et extraits dans le répertoire de qualification.
- Smoke final sur le code `9aa4689` : Windows trois cycles, activations concurrentes sans doublon, collecte continuée,
  préférences privées identiques ; protocole-ready470/371/309ms dans ce seul essai.
  Linux réutilisation de fenêtre, capture réelle, fermeture Flutter avec Rust vivant, puis arrêt natif exit0.
  Bundle Linux final33,36Mio,14fichiers ; pas un package public final.
- Préservation finale : binaires MSI, préférences stables et identité de notification installée identiques ; préfixe historique original conservé.
  **Deux journaux d'intention stables ont évolué pendant la session**, auteur exact non établi ; verrou propriétaire actif non lisible.
  Ils ont été laissés intacts, sans restauration forcée ni interruption du tray MSI/de son enfant.
  Ne pas convertir ce contrôle en affirmation que tous les états de protection ou les neuf fichiers sont inchangés.
  Les notifications TEST et leurs seules métadonnées créées ont été retirées après sauvegarde ; catalogue OS non modifié.
  Un défaut de durée de vie COM du seul outil jetable de nettoyage a été corrigé, vérification finale lecture seule exit0.
- Preuves privées : `%LOCALAPPDATA%\MicCamWatchQualification\flutter-b2-20261029-082656`
  et `/home/roman/mcw-qualification/b2-20261029-082656`.
  Conserver JSON/manifests, journaux et captures ; revue confidentialité avant toute diffusion de ces traces contenant chemins/processus.
- **Point de reprise B3, après validation du propriétaire** : Linux accéléré/SNI et vraies interactions préférences ;
  builds/exécution macOS Intel/Apple Silicon et Linux ARM64 ; contrats pause UI/autostart/diagnostics/historique.
  Commandes caméra/lock, restauration individuelle des sourdines initiales et packaging/updater Flutter restent séparément à qualifier.
  Aucun remplacement des anciennes interfaces ni annonce de parité complète, compatibilité ARM physique ou publication0.21.1.

## 2026-10-30 — Tranche B3 en cours : pause sûre, Linux Mint et CI Flutter

- Reprise vérifiée sur `570c40b`, arbre propre,8worktrees et stash conservés ; main/origin73369ab et tag stable inchangés.
  Référence locale non destructive des binaires, préférences, journaux et identité native enregistrée sous
  `%LOCALAPPDATA%\MicCamWatchQualification\flutter-b3-20261030-081807`.
- Décision propriétaire : aucun Mac physique/manual ne sera disponible ni exigé comme condition bloquante.
  Maximiser compilation, tests et exécution graphique automatisée des deux architectures sur runners ; chaque absence de preuve restera explicite.
- Contrat B3 : protocole4 sans shim ; commandes `notification_pause` / `pause_30_minutes` et `resume`.
  Comparaison sous verrou du timestamp natif exact observé, calcul serveur30min, sauvegarde/relecture de la vraie échéance ;
  reprise ne réactive jamais le master notifications. Paramètres arbitraires et état optimiste interdits.
- Quatre périmètres séparés : Linux/SNI/bundle, macOS/workflow, Flutter/7langues, inventaire CPU.
  Optimisation candidate limitée au filtrage UTF-16 des noms DLL avant allocation, sans raccourcir l'énumération ni modifier les signatures.
  Aucun gain B3 revendiqué avant mesures appariées.
- Écart Linux démontré : binaire B2 Kali exige GLIBC2.39, donc pas de compatibilité Mint21/Ubuntu22.04 annoncée.
  Préparation d'un vrai build Ubuntu22.04 x64 jetable avec audit ELF, bundle isolé et protocole Mint ; WSLg ne remplace pas le PC Mint.
- **Autorisation supplémentaire reçue pour une branche CI B3 dédiée seulement**, après validation locale et contrôle des automatismes.
  Aucune autorisation de push sur main, tag/release/site ou remplacement MSI. La CI publique actuelle ne contient pas encore ce frontend.
  Pont manuel préparé dans CI existante vers workflow réutilisable ; aucun déclenchement push/PR/tag du nouveau workflow.
- Inventaire Cloudflare API indisponible sans token dans cette session ; aucune connexion/condition contournée.
  Prévention documentée : préfixe `[CF-Pages-Skip]` et marqueur `[skip ci]` sur le seul commit de tête envoyé,
  pour omettre Pages et les workflows push, puis dispatch manuel B3.
  [Cloudflare officiel](https://developers.cloudflare.com/pages/configuration/git-integration/github-integration/#skipping-a-build-via-a-commit-message) ;
  [GitHub officiel](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/skip-workflow-runs).
- Premiers contrôles Windows réussis : check tous targets, Clippy strict,46tests desktop et49tests plateforme, build Rust release.
  Flutter : analyse sans erreur ;58tests réussis et un nouveau test clavier corrigé pour disposer son timer de fixture avant vérification,
  puis cas ciblé réussi et build graphique release réussi. Ce n'est pas encore une validation des gestes réels pause/reprise.
- Checkpoints locaux `3cf3a06` (contrat/UI pause) et `1fe5640` (filtrage DLL et compteurs) ; compilation/macOS/Linux encore à qualifier.
  Parse Python, YAML manuel/read-only, scripts shell et fixtures natives vérifiés sans action matérielle.
- Revue sécurité : ancien harness AppKit pouvait capturer tout l'écran si autorisé. L'appel B3 passe maintenant obligatoirement
  `--no-screen-capture` ; seul le cache de vues possédées et la capture Flutter par windowID restent autorisés. Pas de capture globale.
- Accès SSH existant identifié en lecture seule : Linux Mint22.2, GPU Intel HD520, compte propriétaire.
  Session graphique encore à établir ; aucune connexion de bureau, installation ou permission modifiée.
- Revue commandes confirmée : CAS/horloge serveur/session/rejeu/pending corrects ; portée des notifications corrigée dans les7langues.
  Sous Windows, master/pause concernent aussi les notifications natives d'opération/erreur ; aucun mécanisme de livraison modifié.
- Session Mint existante identifiée : X11 via XRDP/XFCE, réutilisable sans nouvelle connexion de bureau.
  GLX rapporte explicitement llvmpipe/accélération non disponible ; ce relevé n'est pas encore une mesure du vrai enfant Flutter.
  Ne pas présenter cette session comme un test Intel accéléré ni changer pilote/permissions pour contourner cette limite.
- Checkpoint CI dédié préparé sur la base de code `48d0d70` ; sources relues et contrôles locaux conservés,
  qualification macOS/Ubuntu hébergée encore non exécutée à ce point. Ne pousser que le ref CI autorisé avec les marqueurs d'exclusion.
- Run CI manuel `41023711442` sur `9773a1b` : SDK officiels et versions validés sur les3runners, mais arrêt avant compilation :
  composant `cargo-clippy` absent du toolchain exact. Correction de préparation : demander explicitement `components: clippy`.
  Aucun build/test macOS ou bundle Linux réussi sur ce run ; les reçus not-completed sont conservés, pas convertis en succès.
- Windows Flutter final :59tests réussis en une passe et build release réussi après correction des textes.
  Premier clic Pause réel a bien sauvegardé l'échéance native ; confirmation accessible en cours, le harness initial cherchait
  à tort des noms atomiques alors que MSAA regroupe les paragraphes et expose les champs sélectionnables via leur valeur.
- Checkpoint B3 suivant : gestes Windows pause/reprise, maître désactivé, refus d'écriture et timeout/reconnexion réellement exercés.
  Deadline native exacte conservée par CAS ; après fermeture, Rust seul soumet/supprime les événements TEST selon la pause persistée.
  Tous les processus possédés sont terminés ; valeurs privées initiales restaurées, sérialisation LF au lieu de CRLF documentée.
- Comparatif propre B3 `cpu-clean-9773` terminé :2tours AB/BA,60s par intervalle, charge et signatures comparables.
  Fermé5,73→5,53% d'un cœur, ouvert8,46→8,00% avec deltas par tour ouverts de signes opposés ; RSS candidat20,77/138,36Mio.
  Ne pas revendiquer un gros gain ni comparer directement aux11,60% B2 d'un autre contexte. Profilage instrumenté séparé terminé, analyse détaillée à suivre.
- Run CI `41024851203` sur `fc941c0` : Rust/Flutter release et tests réussis sur Ubuntu22.04 et les deux Mac.
  Ubuntu53desktop+5settings ; chaque Mac52desktop+5settings. Échec ultérieur de qualification conservé :
  Linux : socket privé165octets, limite native104 non modifiée ; HOME imbriqué du harness corrigé et feuille privée raccourcie.
  macOS : vrai entitlement `get-task-allow=true` injecté par Xcode ; Release désactive maintenant uniquement l'injection de base,
  suivant la recommandation Apple, sans assouplir l'assertion ni le sandbox. Aucune réussite runtime Mac n'est encore affirmée.
- ZIP des artefacts Mac ne préservait pas les liens de frameworks ; correction d'archivage tar natif, SHA256 et contrôle du sceau
  après extraction préparée. Préparer les deux préfixes avant inspection conserve aussi le payload instrumenté en cas d'échec.
  Bundle Ubuntu du run2 intégralement vérifié (19fichiers, ELF, manifeste, SHA256, tar) ; transfert Mint encore à effectuer.
- Run `41028974295` sur `deef4ec` : builds/tests toujours réussis, debug entitlement supprimé, inspections architecture/sandbox/sceau
  et restauration tar des deux builds Mac réussies. Session Aqua disponible ; premier lancement arrêté par le même garde de chemin :
  le socket dépend de **XDG_DATA_HOME**, pas de XDG_RUNTIME_DIR. Le harness Mac utilise désormais un nouveau petit arbre0700 sous
  `~/mcw-qualification`, propriétaire vérifié ; aucune limite ni permission native assouplie. Nouvelle exécution encore nécessaire.
- Bundle Ubuntu `fc941c0` transféré et SHA256 vérifié sur Mint22.2 dans un préfixe neuf, sans toucher au mcw stable1406412.
  Filtre tar Python avait ignoré les modes de répertoire et hérité de l'umask SSH775 ; seuls les8répertoires extraits ont été
  remis à leurs modes déclarés700. Aucun parent ni droit système modifié ; préfixe et19sommes de contrôle inchangés.
- Deux défauts du contrôleur Linux reproduits sans mutation : `gdbus` interprétait la profondeur `-1` comme une option ;
  ajout du terminateur `--`. Locale française masquait «Version needs section» et donnait à tort des versions ELF vides ;
  seules les sorties de commandes analysées sont maintenant forcées en locale C. Vérification réelle : mcw/runner GLIBC2.34,
  moteur Flutter2.18, GLIBCXX3.4/CXXABI1.3 ; aucune réduction des plafonds. Stdout/stderr d'échec sont désormais conservés.
- Mint : vrai SNI possédé, activation et première fenêtre Flutter1040×700 réelle affichée, logo et état inconnu/dégradé honnêtes.
  Le vrai enfant possède des threads llvmpipe : rendu logiciel confirmé. Capture claire examinée, pas de claim Intel accéléré.
  Automatisation préférences encore bloquée sur AT-SPI («Settings» indisponible), après fermeture propre0 et zéro UI possédée.
  Toolkit-accessibility existant=false ; ne jamais activer globalement ou modifier les réglages utilisateur pour contourner le test.
- Profilage Windows séparé : fermé54vérifications WinVerifyTrust baseline/candidat en60s ; candidat46410entrées de modules
  fraîches énumérées,162noms décodés. Évitements de décodage = preuve de mécanisme, pas gain CPU mesuré ; coûts inclusifs non additifs.
  Vérifications de confiance restent fraîches, coût résiduel important ; latence de capture contrôlée non mesurée faute d'action matérielle autorisée.
- Historique de notifications TEST vidé puis relu0 ; seule la clé AUMID Qualification créée par la mission retirée après contrôle exact
  des2valeurs et absence de sous-clé. Identité MSI strictement identique ; catalogue Windows géré par l'OS non modifié.
- Diagnostic AT-SPI établi en lecture seule : registre partagé Mint démarré sur un ancien affichage, échecs de cookie X11
  «Invalid MIT-MAGIC-COOKIE-1 / Cannot open default display» concordant avec le diagnostic10:19.
  Pas de mauvaise propriété transfer-full et pas de preuve d'un défaut de fenêtre Flutter ; ne pas réparer le registre utilisateur.
- Nouveau fixture **de qualification seulement** : bus/registre AT-SPI officiels, adresse privée, EXTERNAL/UID et racine0700,
  affichage déjà autorisé hérité ; vrai bus de session/SNI conservé. Deux processus possédés, journalisés, bornés et arrêtés en finally,
  sans service d'activation ni enregistrement de session. Pas de lecture/log de cookie ni réglage global ; seules les applications
  de qualification reçoivent l'adresse AT-SPI. Contrôleur par exercice pour éviter un cache libatspi traversant les thèmes.
  Ce code est sauvegardé avant exécution ; succès réel du fixture et des préférences encore à établir.
- Capture Mint a révélé un texte Windows «release all» inapproprié sur Unix et un pseudo-défaut attendu d'instantané Windows.
  Contexte moteur validé maintenant passé au panneau ; commande Windows non proposée sans contexte Windows, confirmation invalidée
  si ce contexte disparaît. Raisons natives Unix conservées, pas de nouveau contrôle matériel ; carte alignée sur toute la largeur.
  Deux tests de transitions/consentement ajoutés, pas de tests permanents recopiant les paragraphes ou comptant des textes.
  LSP Dart indisponible dans ce worktree ; références inventoriées et unique appel main.dart migré, analyse/tests/build en cours.
- Contrôle Windows stable après les essais : binaires MSI, settings, fichiers de protection/verrous strictement identiques.
  Historique enrichi pendant la session mais préfixe initial intégralement conservé ; aucune troncature/restauration forcée.
- Mint22.2, bundle Ubuntu figé `fc941c0` : six cycles réels réussis (3clair/3sombre), SNI, même enfant sur activations,
  préférences sons/pause/reprise, refus natif de lecture, restauration, arrêt0 et zéro Flutter résiduel.
  Bus/registre AT-SPI privés réellement fonctionnels puis arrêtés ; course normale de disparition `/proc` corrigée uniquement
  dans le contrôleur. Archive privée vérifiée `mint-first-qualified-evidence.tar.gz`,
  SHA256 `edf4611cf163f214507881d0e73b44b789db689cb6482faacb05968e30f94175`.
- Mint XRDP/llvmpipe : RSS final Rust10,11–10,17Mio, ouvert327,73–334,76Mio ; PSS296,05–303,19Mio ouvert.
  CPU échantillonné24,17–26,21% ouvert,1,45–1,50% fermé ; borne inférieure excluant les enfants très courts entre échantillons.
  Premières fenêtres mappées4,72–4,84s, réouvertures0,97–1,21s ; aucun résultat accéléré revendiqué.
  Requalification du produit `7e794d2` et capture d'un dashboard normal après restauration encore à faire.
- Run `41037181504` (`7e794d2`) : builds/tests des3cibles réussis ; Mac ARM, Flutter réel release puis instrumenté,
  IPC positif,3activations, fermeture sans moteur résident, réouverture et mort du propriétaire réussis.
  Fixture natif AppKit échoue ensuite en exigeant à tort le basculement d'une préférence désactivée :
  correction du test pour conserver l'état autoritatif et exiger zéro événement ; aucun garde de production assoupli.
- Mac Intel : release/IPC positif réussis ; premier lancement instrumenté,3activations/capture/fermeture réussis ;
  la réouverture n'atteint pas Ready. Cause non établie. Nouveau reçu persiste le PID avant attente et tente un échantillon
  en lecture seule du seul enfant encore possédé, sans entitlement de debug, permission ni augmentation du timeout.
- Replay **runtime seulement** préparé : mêmes produits du run41037181504, IDs et SHA256 ZIP/TAR figés, source produit7e794d2,
  source du contrôleur séparée. Manifeste/ancêtre/diff/API vérifiés avant exécution ; aucune recompilation Rust/Flutter de production.
  Seul le producteur de fixtures natif Rust/Swift peut être compilé. Nouveaux contrôles CI encore non exécutés à ce checkpoint.
- Contrôles locaux actualisés : analyse Flutter sans erreur,61tests et build Windows release réussis ; 19hashes du vrai bundle
  Linux relus par le restaurateur. Capture PrintWindow actuelle vide malgré47nœuds UIA réels et fenêtre possédée au premier plan :
  diagnostic de capture en cours, aucune image vide présentée comme validation visuelle ni repli vers le bureau entier.
- Checkpoint `4cabb1b` : replay runtime uniquement du run41037181504 déclenché sur la branche CI autorisée.
  Run `41066470241` : ARM passe les deux vrais cycles release/instrumenté et le fixture AppKit corrigé ;
  Ubuntu passe SNI,12cycles combinés lifecycle/préférences et fermeture des fixtures. Builds/suites initiaux explicitement hérités,
  étapes de recompilation affichées skipped ; aucune reconstruction ou nouvelle publication de produit.
- Mint produit final figé `7e794d2` :6nouveaux cycles, préférences/pause/reprise/refus/restauration réussis, zéro UI/fixture résiduel,
  arrêt des2propriétaires0. Archive SHA256 `5c8408f4cd75eb279164a4f6fb5d386e5c897a540735fa30db05157fcd2d27c9`.
  Captures Overview clair/sombre normales réellement examinées après relecture de la restauration.
  RSS fermé9,97–10,24Mio, ouvert325,40–329,76Mio ; PSS ouvert293,77–298,12Mio. CPU ouvert23,07–23,18%, fermé1,60%,
  toujours borne inférieure des processus échantillonnés et rendu llvmpipe. Ready initial1,41–1,49s mais fenêtre mappée4,86–4,97s :
  ne pas confondre acquittement protocole et premier affichage utilisable. Réouverture mappée0,98–1,09s, arrêt172–234ms.
  Stable Mint PID1406412, binaire, absence initiale des préférences/protections et historique strictement préservés.
- Windows final7e794d2 :3réouvertures,10activations même enfant, crash du seul enfant et relance, collecte continue et fermeture0
  de tous les PIDs possédés réussis. Ready initial580ms, réouvertures179/193/204ms ; RSS22,97Mio fermé/136,10Mio ouvert
  sur cette passe descriptive, distincte du comparatif CPU contrôlé. Captures WGC des seuls HWND possédés clair/sombre réussies
  et examinées ; identité PID/FILETIME/chemin tenue avant/après, aucun picker/capture globale/permission, PrintWindow vide conservé comme échec.
- Intel replay conserve l'échec de Ready à la réouverture. Échantillon valide de l'enfant2960 sous owner2907 :
  une seconde dans NSApplication→WindowServer/get_session_port, avant Flutter. Une autre ligne Impeller apparaît ensuite sans PID/temps :
  pas de preuve d'un blocage permanent du runner ni d'une cause unique. READY_TIMEOUT=15s, IO_TIMEOUT=5s inchangés.
  Contrôle passif timestampé session/AppKit en préparation ; aucune permission ni garde de production modifiée.
- Bundle Mint autonome préparé hors dépôt : `miccamwatch-b3-mint.tar.gz`,13501124octets,
  SHA256 `d17595f5d910a644872bc5ecaaa2f477b2dea7a06a3c52a7a6b7e001d3736254`.
  Archive produit originale inchangée + contrôleurs4cabb1b séparés + helper vérifié + provenance/SHA256/notice française.
  Extraction réelle dans un autre préfixe privé Mint,6hashes wrapper,19hashes produit et `linux.py check` réussis ;
  pas de nouvelle installation ni répétition inutile des6cycles déjà validés sur les mêmes octets.
- Prochain contrôle **Intel seulement**, sans rebâtir ni relancer les jobs ARM/Linux verts : sondes passives précompilées
  CGSession/NSScreen et NSApplication.shared, avant ouverture/après fermeture/pendant réouverture. Processus de diagnostic
  propres, bornés, sans fenêtre, permissions, entrée injectée ou commande produit. Échantillon tardif possédé et journal
  horodaté ajoutés uniquement à ce mode. Ces contrôles peuvent perturber le timing/contexte ; un succès conditionnel ne
  clôturera pas à lui seul les échecs Intel non instrumentés. Résultat encore non exécuté à ce checkpoint.
- Contrôle Intel ciblé `41070188421` sur `e2f3be8`, mêmes produits7e794d2 : sondes sans Flutter réussies avant ouverture
  (session274ms, AppKit214ms), puis toutes deux sans réponse dans leur fenêtre bornée après fermeture et pendant réouverture.
  Sonde AppKit et enfant Flutter attendent get_session_port/WindowServer ; sonde session attend SLSessionCopyCurrentDictionary.
  Enfant5230 encore dans ce bootstrap lors du prélèvement tardif,11,19s après création. Toujours échec Ready15s ;
  pas de deuxième ligne Metal dans cette séquence horodatée. Les processus de contrôle sont possédés, arrêtés et reapés.
  ZIP artefact12984942123 vérifié `e9326e141db6eb3175d7c717a6b0ad677e84dddeaa142e767151eed04b005d71`.
- Conclusion prudente : défaut de réactivité du contexte/service graphique dans cette expérience, pas seulement Dart/IPC.
  Le déclencheur OS/image/fermeture/contexte de lancement n'est pas établi ; ne pas prétendre que tous les runners Intel
  sont fondamentalement incapables. Aucun correctif produit sûr démontré, donc aucune permission/sandbox/timeout/signature assoupli.
  Inspection : propriétaire Mac de développement sans DesktopHost permanent, contrairement au tray opérationnel.
  Prochain discriminant : qualification native menu bar persistante avec mutations désactivées et contrôle fermeture AppKit sans Flutter.
  AppKit de secours conservé, pas d'exigence de Mac physique ; l'échec Intel reste explicitement ouvert.
- Fin B3 pour validation propriétaire : Windows commandes/erreurs/cycles/captures, Mint6cycles/captures/bundle et ARM CI réels acquis ;
  comparaison CPU fermée5,73→5,53%, différence modeste, pas résolution générale. Linux accéléré/Wayland/ARM Flutter et parité complète
  encore non qualifiés. Rapport détaillé `docs/FLUTTER_B3_QUALIFICATION.md`, matrice actualisée, preuves privées indexées et hashées
  dans `%LOCALAPPDATA%\MicCamWatchQualification\flutter-b3-20261030-081807\B3-EVIDENCE-INDEX.json`.
  Windows MSI/settings/protections/verrous inchangés ; préfixe initial de l'historique conservé, identité installée inchangée.
  Mint stable strictement préservé. Main73369ab, tag06d800b→3d74686,8worktrees et stash conservés. Aucun changement site/README,
  aucune release/déploiement ni push sur main/branche de travail. Seule la branche CI explicitement autorisée a été poussée.
  Arrêt après sauvegarde documentaire ; attendre validation B3 avant migration des protections/installations ou publication.

## 2026-10-30 — Tranche B4 en cours : consultation réelle, autostart et hôte macOS permanent

- Reprise vérifiée sur `14ae566`, branche `work/flutter-hybrid-0211` propre ; main73369ab, tag stable,8worktrees et stash préservés.
  État initial et empreintes du MSI/settings/protections archivés dans
  `%LOCALAPPDATA%\MicCamWatchQualification\flutter-b4-20261030-202725`.
- Contrat Flutter IPC5 sans shim4 : nouvelle voie de consultation authentifiée, bornée et indépendante des commandes micro/préférences.
  Le protocole du helper AppKit reste4. Historique lu depuis les vrais JSONL schema3, pagination par curseur opaque,
  fichiers ouverts bornés, verrou court, résultats incomplets explicites ; aucun historique reconstruit à partir des snapshots.
- Flutter : écrans Historique/Diagnostics et carte démarrage automatique, sept langues, thèmes communs, confirmations et états périmés.
  Autostart : contrôle natif du mode opérationnel, comparaison de l'état observé, verrou existant et relecture effective.
  **Revue en cours : l'ownership Unix doit être renforcé pour la portée « cette installation » avant toute qualification de cette écriture.**
  Aucun autostart quotidien ni périphérique modifié.
- macOS : qualification préparée avec le vrai DesktopHost AppKit permanent et le même superviseur Flutter entre les fermetures.
  Actions sensibles désactivées ; menu/NSStatusItem réellement interrogés ; sondes possédées, pas de clic physique revendiqué.
  Les nouveaux builds et cycles Intel/Apple Silicon n'ont pas encore été exécutés à ce checkpoint.
- Mint : inventaire en lecture seule du vrai PC ; session utilisateur XRDP/X11 `:10`, llvmpipe et accélération=false.
  GPU Intel i915 présent, mais nœuds de rendu non accessibles à cet utilisateur ; siège local=greeter LightDM, pas de session utilisateur autorisée.
  Ne changer ni groupes, pilotes, permissions ni login. Aucun résultat accéléré revendiqué.
- ARM64 : pas d'archive SDK Linux ARM64 officielle3.47.8 ; bootstrap depuis la révision Git Flutter officielle exacte et artefacts
  Dart/GTK AArch64 officiels préparé sur runner `ubuntu-24.04-arm` jetable. Empreintes mesurées distinctes d'une signature ou SHA upstream inexistant.
  Build Flutter/Rust, suites et audit ELF prévus ; aucune nouvelle qualification ARM64 exécutée à ce checkpoint.
- CPU Windows : réutilisation d'un seul tampon de chemin UTF16 par passe caméra, sans cache de confiance ni changement de fréquence.
  Gain non encore mesuré ; signatures et identités restent relues. Ne pas annoncer d'amélioration avant comparatif apparié.
- Premières vérifications B4 : `cargo check --all-targets --locked --features windows-tray,flutter-desktop` réussi après correction
  de l'import de Policy ; `flutter analyze --no-pub` sans erreur ; parse des quatre scripts Python et YAML CI réussi.
  Suites, builds release, nouvelles captures et mesures encore à exécuter. Checkpoint volontairement WIP, pas validation finale.
- Reprise : résoudre l'ownership autostart Unix ; terminer les suites/clippy/builds locaux ; pousser seulement le ref CI déjà autorisé
  avec `[CF-Pages-Skip] [skip ci]` et dispatch manuel frais (pas replay B3), analyser les reçus Mac/Linux/ARM, puis gestes/captures Windows/Mint.
  Aucun push de la branche de travail/main, tag, release, déploiement ou remplacement du stable autorisé.
- Sauvegarde WIP locale `79b35db` effectuée avant les opérations longues. Revue indépendante ensuite :
  correction de l'ownership Unix (registre et journal d'une autre installation préservés, y compris cible absente),
  séparation des changements de présentation/observation Unix, suppression du profil par défaut trompeur dans le diagnostic.
  Les deux revues ciblées confirment les corrections à l'inspection ; ce n'est pas une preuve d'exécution Unix.
- Contrat strict : un test a révélé que les variantes Serde unitaires ignoraient les champs supplémentaires ;
  elles sont maintenant des variantes struct sans champ, sans élargir les opérations acceptées.
  Tests Windows réussis :50desktop,11historique,5autostart,19plateforme ; Clippy strict tous targets et build Rust release réussis.
  Flutter :88cas réussis lors de la suite initiale ; échec de fixture clavier (timer de requête non terminé), puis cas ciblé corrigé réussi.
  Analyse et build Windows release réussis ; avertissement MSBuild sur le worktree sous Temp conservé, pas masqué.
  Formatage limité aux nouveaux modules B4. Aucun test physique ni changement de protections/autostart utilisateur.
- CI fraîche `41120973121` sur `d7f778a` : Ubuntu22.04 x64 réussi (build/suites/package et runtime hébergé).
  Les deux Mac échouent avant build release sur un champ `details_label` réservé à Linux dans la nouvelle vue de qualification ;
  champ maintenant gardé par son cfg Linux, sans modifier les interfaces opérationnelles.
  ARM64 : source/Dart officiels et MD5 vérifiés, puis garde trop étroit refusant la licence à la racine du ZIP.
  Extraction désormais limitée au seul sous-arbre `dart-sdk/*` ; métadonnées conservées dans le ZIP original, jamais déversées dans le cache.
  Sonde privée : métadonnées légitimes acceptées ; chemins absolus, traversées, séparateurs ambigus, NUL et liens symboliques refusés.
- Reprise après interruption : aucun processus B4/benchmark encore actif. Comparatif CPU arrêté avant fin du premier tour ;
  données partielles et empreinte conservées séparément, pas de gain ni comparaison complète revendiqués.
  Prochain run CI limité aux trois cibles échouées ; bundle x64 déjà vert conservé, pas de reconstruction inutile.
