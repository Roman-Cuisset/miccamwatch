# Consolidation de l’étape 4 — reprise du 26 novembre 2026

Ce suivi complète [l’audit existant](HYBRID_STAGE4_AUDIT.md) et le
[transfert Neon](HYBRID_STAGE4_NEON_HANDOFF.md). Il ne rouvre pas l’audit des
20 domaines. L’étape 5 et la publication restent exclues.

## Inventaire et conservation

Inventaire en lecture seule avant compilation, sans suivre les jonctions.
Le disque C: contient 475,72 Gio, dont 39,07 Gio libres initialement.
L’accès au volume E: n’était pas disponible ; aucune taille n’est inventée.

| Ensemble | Taille observée en Gio | Nature et décision |
| --- | ---: | --- |
| `miccamwatch/target` | 69,836 | Ensemble mixte, **pas un cache global** ; contient le worktree Neon et des preuves |
| `target/debug/incremental` | 15,051 logiques | Seule suppression explicitement autorisée par l’utilisateur ; effectuée après contrôle du chemin absolu et des compilations actives |
| `target/debug/deps` | 15,640 | Résultats Rust régénérables, conservés |
| `target/debug/build` | 1,031 | Résultats de génération Rust, conservés |
| `target/codex-v022/target` | 5,375 | Anciennes compilations, conservées |
| `target/codex-v022/debug` | 3,492 | Anciennes compilations, conservées |
| `target/release/deps` | 2,579 | Compilations Release, conservées |
| `target/mcw-hyperv-lab-inventory/recovery-build` | 4,767 | Construction de qualification de récupération, conservée |
| Médias Hyper-V dans l’inventaire | 16,86 | ISO officielle et fichier `.iso.ranges.part` de 8,43 Gio chacun ; doublon possible, intégrité comparée non établie, aucune suppression |
| Worktree de développement complet | 5,577 | Sources indispensables et sorties conservées |
| Son `target/debug`, puis `target/release` | 3,685 / 1,260 | Régénérables, conservés |
| Son dossier `desktop/flutter` | 0,624 | Sources, caches et sorties à distinguer avant un éventuel nettoyage |
| Stockage dédié Hyper-V | 45,021 | VHDX de base 20,160 ; AVHDX 11,759 / 9,261 / 3,825 ; **tous conservés** |
| `C:/taily-build/flutter` | 1,768 | SDK d’un autre projet, conservé |
| `desktop` du worktree Neon | 0,292 | Travail intégré et preuve locale, conservés |

Le nettoyage ciblé a libéré **13,30 Gio réellement alloués**, passant à
**52,38 Gio libres** immédiatement après suppression. La taille logique et
le gain physique ne sont pas assimilés. L’espace libre diminue normalement
avec les nouvelles constructions. Aucun `cargo clean`, `git clean`, prune,
suppression de worktree ou nettoyage Hyper-V n’a été effectué.

Les journaux MSI, empreintes, archives CI, benchmarks, sources et checkpoints
restent indispensables jusqu’à la qualification finale. Les compilations
régénérables et les archives devenues redondantes pourront être proposées
séparément après publication ; elles ne sont pas supprimées par cette mission.

## Intégration Neon

Le développement était propre à `16d301245626cd63744c676aa64dbfd1b1be62be`.
Le worktree Neon était propre à
`3e07de07ee83ecabb9ae998f9b44938e9b0f6b58`. La revue du diff a porté sur
les rafraîchissements des contrôleurs, reconnexions et états IPC, les profils
réellement appliqués, les confirmations, erreurs, thèmes et l’accessibilité.
Elle n’a pas révélé de blocage à l’intégration des six commits :
`f50b477`, `22ffd98`, `a256748`, `9baf178`, `c14c280`, `3e07de0`.

L’intégration est un fast-forward, avec les deux commits documentaires.
La branche et le worktree Neon, les autres worktrees et le stash sont conservés.
Le commit mécanique `7bf4862` applique ensuite rustfmt séparément des changements
fonctionnels. Le formatage était nécessaire aux contrôles généraux.

Les résultats Neon restent ceux de leur révision : 129 tests Flutter, analyse
sans problème, sept langues, six pages, deux thèmes, clavier et zoom 200 %.
Les mesures AB/BA à cadence IPC 2 s indiquent 0,858 → 0,351 % CPU,
131,8 → 129,8 Mio RSS, 129,1 → 127,5 Mio privés et 344,9 → 319,3 ms
d’ouverture à chaud. Aucun gain CPU concluant à 5 s, ni gain RAM significatif
n’est revendiqué. Les benchmarks ne sont pas répétés sans nouvelle question.

## Corrections consolidées

- `e5facd1` : action macOS `close_application` distincte de la terminaison
  forcée, confirmation à usage unique, identité libproc retenue, UID réel,
  effectif et sauvegardé, session, exécutable et instance AppKit corrélés.
  `NSRunningApplication.isEqual` protège l’identité ; le succès exige
  `isTerminated`. Une application peut refuser ou attendre un document non
  enregistré. Aucun repli par signal PID, sudo, SIP ou permission détournée.
- Une fixture graphique ad hoc avec Hardened Runtime refuse d’abord la
  fermeture, puis accepte. Le test vérifie également une naissance périmée,
  une cible protégée et la sortie réelle. Il ne reproduit pas l’application
  Vorssaint et ne prouve pas une réutilisation réelle du PID par le noyau.
- Le helper de notifications installé est copié dans
  `Contents/Helpers/MicCamWatchHelper.app`. Son exécutable, plist et ressources
  signées doivent correspondre exactement aux octets embarqués dans Rust.
  Les notifications et lectures d’autorisation utilisent ce chemin stable.
  Les installations de développement conservent leur helper privé temporaire.
- Le signal d’arrêt est relu immédiatement avant `play_chime()`, avec un test
  ciblé. La cadence de surveillance et la file bornée sont préservées.
- La disparition d’une caméra précédemment observée crée une lacune même si
  une autre caméra reste présente. Un test START/disparition/reconnexion/STOP
  vérifie l’absence de STOP inventé. Aucune identité de client n’est fabriquée.
- `3496cc0` : persistance des rectangles Windows/Linux, contrôle des valeurs
  enregistrées et retour dans la zone de travail. Windows traite
  `WM_DISPLAYCHANGE`, les changements de zone utile et de DPI. Linux ajuste
  également ses minimums lorsque la zone de travail rétrécit.
- `7daa359` corrige la comparaison du nouveau test caméra sans ajouter une
  égalité artificielle au type public des événements.
- `4f48f31` accepte les deux dispositions des sorties Cargo observées, tout
  en exigeant les bindings natifs, l’architecture ELF et le plafond glibc.
  Le paquet macOS et son contrôle installé recherchent et vérifient le même
  helper signé dans ces deux dispositions.
- `8cc2193` introduit le contrôleur VM ciblé et un baseline MSI hors ligne
  épinglé. Les gardes VM/compte standard et l’observateur administrateur
  distinct sont conservés. Le passage positif ne prétend pas refaire la
  campagne des pannes de l’étape 3.

La [documentation AppKit sur terminate()](https://developer.apple.com/documentation/appkit/nsrunningapplication/terminate%28%29)
exige d’observer la sortie, et la
[documentation processIdentifier](https://developer.apple.com/documentation/appkit/nsrunningapplication/processidentifier)
déconseille de comparer des processus par le seul PID.

## Preuves de cette reprise

| Vérification | Révision et résultat |
| --- | --- |
| Rust ciblé Windows, outils sans effets matériels | 11/11 réussis après les premiers correctifs |
| Clippy strict Windows `--lib --tests`, `windows-tray,flutter-desktop` | Réussi |
| Flutter officiel 3.47.8 `analyze --no-pub` | Aucun problème |
| Flutter `test --no-pub` | 131/131 après `a0b40f0`, dont refus de faux succès, annulation de fermeture normale et retrait d’autorité à la reconnexion |
| Qualification Flutter Unix consolidée | [45770903856](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45770903856), source `3496cc0` ; tests caméra macOS initialement non compilables, corrigés dans `7daa359` |
| Qualification MSI exact | [45770908439](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45770908439), source `3496cc0` ; en cours lors de cette entrée |
| CI générale initiale | [45770913849](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45770913849), source `3496cc0` ; erreurs macOS du nouveau test et chemin historique des bindings Linux, corrigées sans supprimer les critères |
| Reprise Flutter Apple Silicon | [45771443063](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45771443063), source `7daa359` ; contrôles natifs réussis, suite graphique en cours |
| CI native corrigée | [45772220951](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45772220951), source `4f48f31` ; en cours |
| Reprise Flutter Intel | [45772225874](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45772225874), source `4f48f31` ; en cours |

### Résultats et corrections complémentaires

Les deux qualifications Flutter Linux de `3496cc0` ont réussi. Celle
d’Apple Silicon de `7daa359` a également réussi, dont la fixture AppKit durcie.
Les anciens échecs ne sont pas effacés par ces reprises.

Le journal Windows hybride de `3496cc0` établit 303 tests Rust réussis, puis
34 tests de maintenance empaquetée réussis. Les **six cas bloqués chez Neon**
réussissent explicitement sur ce runner isolé : namespace du profil réel,
marqueur de redémarrage et quatre scénarios de transaction portable. Cette
preuve distingue la restriction du bac à sable d’une régression du produit ;
aucun de ces essais n’est lancé sur le profil quotidien.

La CI générale a révélé trois avis de sécurité sur le verrouillage ancien :
[cookie 0.18.2](https://rustsec.org/advisories/RUSTSEC-2026-0594),
[url 2.5.8](https://rustsec.org/advisories/RUSTSEC-2026-0592) et
[zip 9.0.4](https://rustsec.org/advisories/RUSTSEC-2026-0548).
`2aae176` verrouille leurs versions correctives 0.18.3, 2.5.9 et 9.0.5,
ainsi que rustix 1.1.6, tokio 1.53.3 et async-executor 1.14.1.
Clippy strict Windows et rustfmt passent après cette mise à jour.
L’étape d’audit des dépendances de la nouvelle CI Windows réussit.
Les avertissements de maintenance des dépendances sans correctif disponible
restent visibles ; aucune exclusion d’avis n’est ajoutée.

La nouvelle candidate de produit est construite depuis `2aae176` :
[Flutter Unix 45774068870](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45774068870),
[Windows MSI 45774075203](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45774075203),
[CI générale 45774081395](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45774081395).
Le MSI antérieur `3496cc0`, SHA-256
`bcdd1e2a5e5cc43024176e5bb4f8eec896b529328bdcbf41f02a3ee979ac87e7`,
est conservé mais n’est pas présenté comme la candidate corrigée.

`a0b40f0` corrige ensuite le programme sonore du harnais général : sa
compilation est confiée à Cargo, avec ses métadonnées de dépendances, plutôt
qu’à une invocation manuelle de rustc qui échouait à retrouver anyhow.
Le test sonore est conservé et le pilote n’est pas distribué. Ce commit ajoute
également le test Flutter d’annulation et finalise les scripts du laboratoire.
Il ne change ni le code des exécutables du produit, ni Flutter distribué,
ni `Cargo.lock` depuis `2aae176`. La
[reprise Unix 45775466491](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45775466491)
porte sur ce harnais corrigé.

Les journaux de cette reprise sont sous
`C:/Users/stcui/Documents/miccamwatch/target/stage4-qualification`.
Une CI échouée reste une CI échouée, même quand des étapes individuelles ont
réussi. Les reçus d’autres sources ne sont pas renommés au HEAD documentaire.

## Laboratoire et limites de clôture

VM vérifiée : `MCW-Win11-x64-Lab`, génération 2, GUID
`cb46ea19-9415-423a-9b89-2459da960886`, SMBIOS
`4B01C3A6-C913-41AC-B759-66148DFCAB64`, Windows AMD64 natif.
La VM était arrêtée puis a été démarrée pour cette mission. Le checkpoint
`01 - Windows 11 x64 propre` et les deux autres checkpoints sont conservés.
La carte virtuelle est déconnectée. PowerShell Direct fonctionne avec
authentification sécurisée en mémoire. L’inventaire n’a trouvé aucun client
MSI ni tâche de test active. Le propriétaire a ouvert le vrai bureau QA.

Les premières préparations ont été refusées par la politique PowerShell
effective `Restricted`, avant toute transaction MSI. Après les accords
explicites du propriétaire du 26 novembre, les seuls processus QA de test et
le processus administrateur d’observation utilisent `RemoteSigned` au niveau
du processus. Les stratégies persistantes, GPO et Windows Installer restent
inchangées. Le contrôleur authentifié reste en attente après un échec et
consomme chaque demande une seule fois, avec reçu et empreinte du script.

Les préparations `071430` et `072314` ont réussi sans MSI. La seconde inclut
le véritable observateur administrateur, avec deux produits absents (1605)
et les trois contextes absents (259). Son code est dans le répertoire d’entrée
protégé en écriture contre QA ; ses reçus sont lisibles par QA sans exécuter
de code de son profil sous le compte administrateur. Ces reçus sont des
**contrôles préalables**, pas une installation, restauration ou désinstallation.

Le contrôleur ne restaure aucun checkpoint. Il sauvegarde les anciens fichiers
du profil QA dans un nouveau dossier du laboratoire avant d’exécuter le
passage positif ; aucune ancienne preuve n’est supprimée. Il refuse un produit
déjà enregistré ou une transaction concurrente. Les opérations utilisent le
jeton interactif limité QA ; l’observation des trois contextes MSI et du cache
utilise le compte administrateur distinct.

**Vorssaint reste non démontré** : une fermeture normale sûre n’équivaut pas à
une terminaison forcée autorisée, et une fixture n’est pas l’application réelle.
**Caméra physique, veille/réveil, bannières avec autorisation persistante,
lecteurs d’écran complets et véritable retrait multi-écran restent à distinguer
des tests logiciels.** Aucun nouveau test n’est demandé à l’ami sur Mac.
Le rendu Linux Xvfb ne qualifie pas un GPU matériel ou Wayland.

L’installation stable de l’hôte n’a servi qu’à des lectures d’empreintes :
`mcw.exe` = `55753140F5F73743FE49399C3FA7A126234EA860029B7F71D673CE80A3AF8FE8`,
tray = `5A0253EA90A77BF7478F6A9F1826290F6A6055723CB12BDF4975B89BDFB1B6FE`.
Son processus tray était PID 18072 au contrôle de cette reprise.
La lecture ultérieure retrouve également la v0.21.0, ProductCode
`{86CDFED5-BA07-4C91-8BC7-8A856198DC33}`, dans la clé Uninstall WOW6432Node
et les InstallProperties Windows Installer du profil hôte. Un contrôle après
la qualification finale reste nécessaire.

**ÉTAPE 4/6 : INCOMPLÈTE** à cette entrée : les campagnes et la VM sont en
cours, et aucun report explicite des preuves matérielles/P0 restantes n’est
considéré comme accepté.
