# Roadmap technique post-v0.16.1

Dernière mise à jour : **2026-10-09**. Responsable : lead architecture/développement.
Base publiée : [v0.16.1 stable/latest](https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v0.16.1), source `7b134e3500860bd185ddc3955107b8082d414e78` ; [CI main](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/37487806861) verte avant création du tag.

Ce document planifie les prochains travaux ; sa création ne clôture aucune phase produit et ne justifie pas de nouvelle release binaire. Les fonctionnalités proposées ci-dessous ne sont pas annoncées comme disponibles. Les complexités sont des estimations de conception, pas des délais ni des mesures.

**Candidat source `0.17.0`, non publié :** Windows-first explicitement retenu pour
la protection persistante. Les corrections candidates annotées ci-dessous ne
réécrivent ni les défauts historiques ni les preuves de `v0.16.1`. Les pins
d'installation restent `v0.16.1` jusqu'à publication ; P1-RAM/P1-RESTORE restent
**En cours** jusqu'à qualification CI native/helpers/matérielle. Les autres lots
demeurent ouverts. Voir le [contrat candidat et ses limites](ARCHITECTURE.md#candidate-0170-windows-first-protection-contract).

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
| P1-RAM | En cours : bornes candidates 0.17.0, mesure Windows locale >15 Mo ; qualification native/helpers restante | Moyenne | Mesures reproductibles de tout l'arbre produit |
| P1-RESTORE | En cours : propriétaire natif Windows et journal par endpoint dans le candidat 0.17.0 | Élevée | Qualification CI native, états opposés/hotplug/restauration |
| P1-HB | Homebrew : à faire | Moyenne | Propriété installation/autostart, validation macOS |
| P1-AUR | AUR : à faire | Moyenne | Dépendances PipeWire et frontière root |
| P1-WG | Winget : à faire | Moyenne | Identité MSI et upgrades contrôlés |
| P1-ARM | Linux ARM64 : à faire | Élevée | ABI/sysroot, installation et tests natifs |
| P1-UPGRADE | Partiel : correction macOS standalone livrée en 0.16.1 ; consolidation restante à faire | Moyenne | Contrats receipt, PATH, root et package manager |
| P2-HOOKS | Retenu, à faire | Élevée | Transitions actives cohérentes, effets bornés |
| P2-NET | Reformulé en métadonnées locales, à faire | Élevée | PID authentifié ; macOS conditionnel |
| P2-LOCK | Reformulé en protection coordonnée, à faire | Élevée | P1-RESTORE et arbitrage des intentions |
| P2-DIAG | Diagnostic retenu ; réparation forcée écartée | Moyenne | Couverture explicite et identité processus |
| P3-STREAM | Flux existant ; adaptateurs à faire | Faible | Contrat événements/santé et versions de schéma |
| P3-PTT | Reformulé en sourdine possédée, à faire | Élevée | P1-RESTORE, intentions et raccourcis OS |
| P3-SOUND | START déjà disponible ; extension à faire | Moyenne | P2-HOOKS : sémantique des transitions |
| P4-ANDROID | Hors cœur, reporté en dernier | Élevée | Projet indépendant et modèle de permissions |

## Phase 1 — Packaging et écosystème de distribution (immédiat)

### P1-RAM — Qualifier la mémoire et borner les accumulations avant extension

**Fonction :** établir une baseline puis supprimer les croissances non bornées avant d'élargir le produit. Priorité aux observations et aux effets existants, pas à une nouvelle abstraction de performance.

**Modules Rust :** `src/platform/linux/pipewire.rs`, `src/watcher/windows.rs`, `src/watcher/unix.rs`, `src/frontends/tray.rs`, `src/frontends/tray/unix.rs`, `src/platform/macos.rs` ; `Cargo.toml`/`Cargo.lock` pour contrôler les dépendances.

**Contraintes historiques de la base, avant correction candidate :** le JSON PipeWire autorisait **16 MiB** de brut puis un `Vec<serde_json::Value>` ; cette limite n'était pas une borne RSS. `stderr` du subprocess n'était pas borné. Le tray Windows avait des canaux non bornés et un thread par notification ; le cooldown du watcher Windows n'était pas purgé. Ces faits de code constituaient des risques, pas une mesure de dépassement. Borner sans tronquer silencieusement les captures : dépassement = santé dégradée/indisponible explicite. Ne pas supprimer les preuves d'identité pour économiser la mémoire.

**Implémenté en source candidate 0.17.0 :** lecture PipeWire streaming bornée,
stdout/stderr et données retenues bornés, deadlines enfant et erreurs de santé
explicites sans faux STOP ; files UI/notifications/caches bornés. `top` conserve
une seule rangée horizontale de six boutons, avec représentation compacte aux
petites largeurs. Cela corrige les risques historiques, sans certifier le RSS.

**Preuve locale, pas CI complète :** comparaison Windows read-only `9fc99dd`/
stable terminée, functional/completed true, cible **15 Mo non atteinte**.
RAM/CPU/latence et limites dans la
[table canonique](ARCHITECTURE.md#local-candidate-evidence-and-remaining-qualification).
Aucun garde/tray actif dans cette mesure ; gaps candidats ≈63 ms, enfants/pics
brefs potentiellement manqués. Tout l'arbre avec gardes/helpers et scénarios
natifs reste à qualifier ; ne pas annoncer « optimisé sous 15 Mo ».

**Validation :** baseline stable/pic et latence sur les trois OS, gros graphes et rafales d'événements, UI lente, enfant bloqué/sortie excessive ; aucune accumulation et aucun STOP inventé en cas de limite. Toute impossibilité de respecter le budget doit être remontée avant ajout de fonctions, pas compensée par une mesure plus flatteuse.

**Complexité : Moyenne**, potentiellement Élevée si le profiling impose de modifier le parsing ou le cycle des helpers.

### P1-RESTORE — Préserver chaque microphone Windows

**Fonction :** rendre la restauration après verrouillage sûre pour plusieurs endpoints et préparer lockdown/PTT.

**Modules Rust :** `src/platform/windows.rs`, `src/frontends/tray.rs`, `src/collector.rs`, `src/model.rs` ; réutiliser les conventions de possession de `src/platform/linux/control.rs` et `src/platform/macos/control.rs`, sans remplacer leurs backends.

**Contraintes historiques :** dans v0.16.0, le tray sauvegardait `mute_state == Muted` dans un seul booléen. Un état `Mixed` devenait donc `false`, ensuite appliqué à tous les endpoints : risque de démuter une entrée initialement coupée. Les originaux par endpoint du backend servaient seulement au rollback immédiat. Exigence conservée : ID stable, original, génération/intention et readback ; une action manuelle récente prime sur une restauration tardive. Déconnexion, nouvel endpoint, redémarrage et échec de rollback ne doivent pas devenir un succès implicite.

**Implémenté en source candidate 0.17.0 :** propriétaire natif de l'intention
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

**Preuves locales :** source `6bc7fd0`, Clippy strict feature, deux suites Windows
(160 lib +1 main, 1 visuel ignoré), build release et huit tests IPC natifs réussis ;
ConPTY sept langues, 120/150/40/20 colonnes, souris/refresh/quit et screenshot
FR150 inspecté. Status d'absence normale et réservations update rebâtis.
Fixture native : suppression de la rétention des pipes du CLI terminé par le
garde détaché, sans aucune mutation physique. Inventaire micro/caméra lu seul.
Release explicite et restauration achevée exigés avant remplacement ; jamais
de release implicite pour updater. Qualification CI/helpers/matérielle restante.

**Sécurité update candidate (tests natifs locaux, CI complète pendante) :** portable Windows
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

**Complexité : Élevée.** Correction intégrée au candidat source `0.17.0`, non publiée ; qualification restante avant clôture P1-RESTORE, sans modifier les releases immuables.

### P1-HB — Formule Homebrew macOS

**Fonction :** installation, mise à niveau et suppression par Homebrew pour macOS 15+ ARM/Intel. Commencer par un tap maintenu, puis soumettre à Homebrew/core si ses critères sont satisfaits ; « officiel » ne signifie pas acceptation déjà obtenue.

**Modules Rust :** pas de collecteur supplémentaire ; contrats de `build.rs`, `src/platform/macos.rs`, `src/updater/unix.rs` et `src/autostart/unix.rs`. Le helper Swift/AppKit existant reste embarqué ; sa compilation exige le SDK/compilateur adéquat, pas un nouveau runtime applicatif.

**Contraintes :** formule versionnée, source/artefacts immuables et checksums ; recettes de build avec dépendances verrouillées. Homebrew possède le préfixe, pas le receipt de l'installateur curl : pas de double updater ni de remplacement des fichiers du gestionnaire par `mcw update`. Valider la cible de LaunchAgent après upgrade sans écraser une inscription externe ou activer l'autostart. Aucune modification Gatekeeper/quarantaine et aucune signature Developer ID fictive.

**Validation :** `brew install/upgrade/uninstall`, CLI et menu natif, conservation des préférences/historique et de l'intention d'autostart sur ARM/Intel.

**Complexité : Moyenne.**

### P1-AUR — Paquet Arch Linux

**Fonction :** `PKGBUILD` et `.SRCINFO` maintenus, construits depuis un tag immuable avec Cargo verrouillé. Pas de deuxième variante binaire/source tant qu'elle n'est pas nécessaire.

**Modules Rust :** contrats de `src/platform/linux.rs`, `src/platform/linux/pipewire.rs`, `src/updater/unix.rs`, `src/autostart/unix.rs`, `src/privacy/linux.rs` ; scripts existants de `installer/` et patch ABI `vendor/libspa` à conserver/évaluer explicitement.

**Contraintes :** dépendances PipeWire/SPA/`pw-dump` déclarées ; outils desktop/sons seulement pour les usages associés. Le package manager possède ses fichiers. L'installation du paquet ne doit ni activer l'autostart ni installer/autoriser silencieusement le helper root et sa policy Polkit. Les chemins privilégiés et receipts actuels ne deviennent pas une seconde convention AUR : payload reviewable et bootstrap administrateur explicite séparés ; tout changement de chemin exige une migration propre.

**Validation :** construction en environnement Arch propre, installation/upgrade/removal, PipeWire réel et PATH ; absence d'action caméra/root et préservation des fichiers utilisateur.

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
| 2026-10-09 | Candidat source 0.17.0 Windows-first P1-RAM/P1-RESTORE | En cours, non publié ; stable/latest reste v0.16.1 | Bornes et propriétaire natif implémentés ; Clippy/tests/IPC/ConPTY Windows locaux, source PipeWire virtuelle externe démutée après 5 s et nettoyée ; mesures Windows read-only >15 Mo (voir Architecture) | CI native complète, helpers Windows vides réels et mesure avec gardes, qualification matérielle/restauration ; dernière correction status pas encore rebâtie ; aucun autre lot clôturé |
