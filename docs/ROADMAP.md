# Roadmap technique MicCamWatch

Dernière mise à jour : **2026-10-22**. Responsable : lead architecture/développement.
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
