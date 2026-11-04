# MicCamWatch v0.22.0 — plan définitif en six étapes

Date : **2026-11-04**. Décision propriétaire : une seule prochaine version publique, **v0.22.0 hybride Rust + Flutter**. Aucun calendrier artificiel. Ce plan n'autorise aucun push, merge, tag, release, déploiement ou modification de l'installation quotidienne.

Source de la planification : [audit exhaustif et matrice P0/P1/P2](V0.22.0_PARITY_AUDIT.md). Les IDs ci-dessous désignent ses critères mesurables, pas des fonctionnalités déjà terminées. Les preuves B1–B5 et la candidate native0.21.1 restent conservées ; les reports automatiques vers0.21.2 du plan RC historique sont supplantés par cette décision.

## 1. Base et règles communes

- Base recommandée après accord étape2 : créer une branche de développement0.22 depuis B5 `b6f820b1bf7ca091b39927a45e01cec83b50fdbf`, y reprendre les commits documentaires de `docs/v022-parity-plan`. Ne pas supprimer/renommer les branches historiques, ni fusionner aveuglément la RC native.
- Garder les correctifs CRT déjà B5, le garde PE durci, la version tray sans effet de bord, l'inspection autostart COM, les pipelines de notifications et le retour Skia macOS épinglé. Les blobs CRT/PE/tests/générateur identiques à la RC ne doivent pas être recherry-pickés.
- Réutiliser de la RC les procédures/bundles de qualification, les contrôles Git et les mesures natives explicitement étiquetées. Ne pas importer sa version0.21.1, ses notes ni sa décision de livrer des fenêtres natives.
- Rust conserve surveillance, décisions de confiance, contrôles/restauration, autorisations, préférences/historique, CLI, notifications/tray, maintenance et supervision. Flutter n'effectue aucun contrôle matériel direct et ne conserve aucun engine résident après fermeture.
- Chaque nouvelle commande : comportement natif → contrat typé et autorité → test de refus/état périmé/erreur/timeout → UI → readback Rust → restauration/persistance/cycle sans UI. Aucun appel `execute`, shell ou endpoint public générique.
- Les quatre issues restent distinctes : accepté, succès confirmé, échec/refus, incertain. Reconnexion/fermeture ne doivent ni rejouer une mutation ni transformer un pending en succès.
- P0/P1 complets avant proposition d'une RC. P2 facultatifs ne bloquent pas la release sauf s'ils révèlent un problème de sécurité, d'accessibilité essentielle ou une régression importante.
- Les anciennes fenêtres natives restent un secours pendant la migration, pas le moyen de déclarer une fonction Flutter terminée. Pas de suppression définitive imposée à0.22.
- Toute action matérielle sur le PC quotidien, tout changement de VM/compte/configuration OS et tout push nécessitent l'accord approprié. Les permissions accordées à d'anciennes missions CI/Hyper-V ne valent pas autorisation permanente.

## 2. Étape2 — finalisation des fonctionnalités Flutter

**Responsable d'intégration : GPT-6.1 Sol / Codex OAuth / xhigh. Démarrage seulement après validation de l'étape1.**

### Ordre d'exécution et dépendances

| Lot | Tâches concrètes et composants | Dépendances / propriétaire |
|---|---|---|
| 2A — contrats | Spécifier verbes/DTO minimisés pour caméra, micro Unix, lock, détails/devices, historique clear, pause bornée, policy/session, maintenance. Étendre `src/frontends/desktop/{commands,preferences,consultation}.rs` et les modèles Dart ; versionner de manière stricte si schéma incompatible. Définir confirmations et capacité OS pour chaque effet. | Avant tous les nouveaux boutons. Sol xhigh obligatoire pour autorité, restauration et admission ; aucune refonte du moteur. |
| 2B — opérations non matérielles | Compléter durées notifications, copier diagnostics, chemins/clear d'historique confirmé, device inventory/explain bornés, consultation/validation de la politique réellement appliquée ; préserver les quatre préférences/profils/autostart B5. `desktop/flutter/lib/*`, `src/history/reader.rs`, consultation/prefs. | 2A ; opérations indépendantes de l'accès matériel. DS-UI/DS-TEST possibles après interface figée et autorisation séparée. |
| 2C — contrôles et lock | Brancher les backends existants micro Linux/macOS, caméra PnP Windows/reprise/Allow/legacy, USB/Polkit Linux, profil/inspection macOS, paramètres lock sur capacités réelles. Passer par workers/owners Rust existants de `tray.rs`, `tray/unix.rs`, `windows_audio.rs`, privacy/platform. Conserver le contrat Windows release-all explicite et les originaux Unix/automatiques. | 2A ; blocage sur un doute de restauration/ownership. Sol xhigh, revue avant action physique. Commencer par doubles de backends et chemins refusés/non disponibles. |
| 2D — fonctions avancées | Détails de processus et action manuelle identité-liée, option d'enforcement explicite et --no-kill, filtres/session/logs déjà disponibles en CLI. Exposer les besoins utilisateur sans shell ou nouveau système de détection. `watcher`, `policy`, TUI authority helpers, bridge et écrans avancés. | 2A/2B ; décisions O06/O07 P0 revues par Sol. La syntaxe CLI/TUI reste intacte ; pas de terminal embarqué. |
| 2E — maintenance et identité | UI installation-aware pour update/uninstall : actions admissibles ou instructions exactes MSI/Cargo/Homebrew/app/helper. Ne pas effectuer de binary-only update si bundle incomplet. Unifier version0.22 dans Cargo/Flutter/About/resources quand la branche de développement est créée ; traduire l'About anglais et compléter les sept langues/navigation. | Contrats 2A + définition du payload étape3 avant d'activer une maintenance hybride mutatrice. Pas de bouton prétendument opérationnel en attendant. |

### Tests indispensables

- Rust : admission session/child, replay/ID/génération/CAS, confirmation absente/incorrecte, mauvaise plateforme, file pleine/worker déconnecté, autorisation refusée, résultat tardif/uncertain. Flutter : vrais comportements des clients et boutons, aucun état optimiste, messages traduits, reconnexion et fenêtres fermées pendant travail accepté.
- Préférences : concurrence avec CLI/native, politique explicite prioritaire, persistence/relecture/restart, master désactivé, pause expirée, échec disque/lock. Autostart : étrangère/opt-out/permission/refus et rollback, pas seulement save puis lecture.
- Micro : pré-sourdines, écriture partielle, readback échoué, départ/arrivée/génération, intention externe et crash ; Windows release-all distinct d'une restauration d'originaux. Tests des backends natifs réutilisés, pas des mocks qui répètent les demandes.
- Caméra : UAC/Polkit/profil pending/refus/annulation, propriétaire/version incorrects, absence de caméra, restauration owned/externe/legacy, USB composite et journaux incomplets.
- Processus : cible protégée/inattribuée, PID réutilisé, confirmation expirée/annulée, Ready/Unknown et règle non Deny jamais tués automatiquement. Aucun processus réel quotidien comme cible.
- History : données réellement enregistrées, rotation/clear concurrent, cursors rejoués/expirés, I/O tronquée et permission refusée ; ne supprimer que l'historique désigné.

### Acceptation et point d'arrêt2

Les besoins O01–D01/A01, M/C/K et fonctions utilisateur de L/U/V ont un contrat et une implémentation Flutter utilisable ; la matrice distingue encore les validations installées/matérielles à faire étape3/5. Aucun P0 de conception en suspens ; fonctions OS indisponibles explicitement inactives. Pas de besoin courant obligeant à ouvrir la fenêtre native. Aucune nouvelle protection ou export facultatif ajouté pour élargir le périmètre. Rapport avec tests, sources, captures réelles disponibles et prochaine autorisation. **Ne pas commencer étape3 sans accord.**

## 3. Étape3 — intégration et sécurité multiplateforme

**Dépendance : contrats étape2 stables.** Le format de distribution peut être préparé pendant2E, mais son activation nécessite ce gate.

| Lot | Tâches / fichiers ou composants | Tests et acceptation |
|---|---|---|
| 3A — Windows installé | `installer/miccamwatch.wxs`, layout `desktop/`, `.github/workflows/release.yml`, garde PE et reçus. Inclure runner, engine, AOT, ICU/assets/licenses ; un seul produit perUser, un lien, PATH/CLI conservés. Adapter le garde MSI/ZIP qui exige actuellement exactement deux EXEs pour vérifier explicitement le troisième runner et tous les composants attendus, pas ignorer les EXEs supplémentaires. | Extraction réelle MSI/ZIP, chaque byte lié au build et tous PE ordinaires/différés ; DLL inconnue/CRT inattendu/asset manquant échouent. Vrai Windows standard interactif jetable, installation/lien/tray/close-reopen/singleton. Tester une VM sans VC **avant** d'y installer des outils pouvant l'ajouter. |
| 3B — Linux x64/ARM64 | Layout préfixe + bundle complet, `installer/install.sh`, ELF/SPA/permissions/receipts, SDK officiel épinglé, helper séparé/polkit/version. Réutiliser x64 Ubuntu22.04 et ARM bootstrap/audit. Déterminer et préserver réellement le plancher glibc2.35/Ubuntu22.04 de la distribution publique ; build ARM24.04 n'est pas preuve de cette compatibilité. | Tous ELF/loader/GLIBC/GLIBCXX/CXXABI, libspa/GTK/PipeWire ; runtime/SNI/IPC/préférences/lifecycle sur x64 et ARM hébergé si disponible. Pas de droits caméra/ptrace/render ajoutés. Plancher non respecté = blocage ou décision propriétaire explicite, pas régression silencieuse. |
| 3C — macOS deux architectures | `installer/macos/package.py`, launcher/menu AppKit permanent, nested app/frameworks/flutter assets, images/IDs et LaunchAgent. Maintenir sandbox/entitlements et Skia opt-out avec3.47.8 ; revalider tout SDK différent. Deux builds natifs, pas Rosetta substitutif. | Runners Intel/Apple Silicon : release/analyse/tests, architecture/minOS15, seals/entitlements, vrai IPC/panneaux/7langues/clair-sombre/plusieurs cycles/single child/menu bar/parent death. Pas de prompt/grant automatique Screen Recording/TCC. Installation/layout/upgrade whole-app et erreurs vérifiés automatiquement quand possible. |
| 3D — maintenance transactionnelle | `src/updater/{download,process,transaction,unix}.rs`, `src/uninstall{,/windows}.rs`, reçus/autostart/installateurs. Versionner l'identité de **tout** le payload sans adopter le journal d'une autre installation. Le client0.21.0 ne sait pas forcément accepter l'archive étendue : établir la procédure de migration depuis ce vrai client. | Upgrade0.21.0→0.22, interruption avant/après chaque swap/commit, rollback UI+Rust cohérent, reçu falsifié, fichiers remplacés, version mixte, processus encore mappés, garde/restauration pendante et autostart étranger. Si ancien client refuse l'archive, fournir parcours officiel installer entier : ne pas promettre auto-update impossible. |
| 3E — suppression et crash | Uninstall porté au bundle complet, coopératif et exactement possédé ; ne jamais supprimer données/policy/history/records. Fermer Flutter proprement puis observer la fin de son arbre, protection indépendante ; différencier close fenêtre, Exit application et owner garde. | MSI/Cargo/portable/managed/helper/Homebrew/app selon règles existantes, données conservées, mauvais owner refusé, rollback/pending empêche removal, un seul raccourci/App entry, aucun enfant orphelin après crash contrôlé. |

### Dépendances, risques et répartition

3A/3B/3C peuvent se préparer séparément après le contrat de payload commun ; **3D/3E dépendent de ces layouts**. Revue de Sol xhigh requise avant toute modification de restauration, provenance, runtime trust ou transaction. DS-PACK peut inventorier les ressources et produire des tests d'identité après schéma figé ; ne décide pas des locks/ownership/rollback.

Risques bloquants : vieux updater pair-only, archives hybrides mal identifiées, ABI ARM, activation/singleton selon contexte OS, version de garde étrangère, installation standard/manager non qualifiée. Les limites matérielles ARM/TCC restent explicitement annoncées ; **aucun test manuel sur Mac physique n'est requis**. Une absence de GUI sur un runner produit « non exécuté », jamais un succès ; un autre runner/adaptation de test autorisée est nécessaire si le gate GUI requis manque.

### Acceptation et arrêt3

Même code Flutter réellement intégré aux cinq distributions, capacités OS différenciées par Rust, aucune perte de la CLI/tray/restauration. Tous les P0 de canal/lifecycle/payload/maintenance ont leurs preuves positives et négatives automatisées ; les contrôles matériels non exercés sont nommés, sans garantie universelle. Rapport d'intégration et limites, **attendre accord étape4**.

## 4. Étape4 — optimisations et finition

Dépendance : une application fonctionnelle et des payloads réels, non un prototype. Source/API stables ; ne pas transformer ce lot en nouvelle migration.

### Travail concret

- Exécuter le protocole CPU/RAM section7 sur la vraie0.21.0 et le futur build0.22. Mesurer séparément Rust fermé, UI ouverte et après fermeture, processus d'aide/garde compris ; coût signature/process inventory/sensor/IPC/rendu séparé.
- Réduire seulement le calcul redondant démontré, les allocations/copies inutiles ou l'ordonnancement bloquant. Garder WinVerifyTrust, révocation/catalogues/policy/identité fraîches ; pas de cache de verdict fondé sur taille/date ou PID seul. Si l'invalidation sûre n'est pas démontrée, ne pas mettre en cache.
- Préserver cadence et qualité de surveillance, comparer latences réelles avant/après. Aucune suppression de vérification ou collecte pour embellir CPU/RAM.
- Étudier le renderer réellement actif sous Mint. Comparer llvmpipe et accéléré seulement si accès déjà autorisé ; pas de modification de groupes/driver/session ou désactivation GPU silencieuse.
- Corriger thèmes, overflow sept langues, About/version, noms/logos/icônes, clavier/focus/contrastes/lecteurs d'écran dans `desktop/flutter/lib` et runners. Contenu commun ; décorations/permissions natives peuvent différer. Pas de nouveau logo, dépendance décorative lourde ou moteur Flutter gardé chaud.

### Tests, acceptation et arrêt4

Paires complètes, traces CPU profilées séparées, latences/ressources/renderer/outliers et processus possédés archivés. Pas de régression importante inexpliquée ; objectifs historiques non révisés en silence. Réutiliser `installer/tests/native_resources.py` : **objectif historique15_000_000octets**, pas une promesse déjà atteinte partout, ni budget à remplacer pour justifier Flutter. Les écarts et le coût fenêtre ouverte restent visibles et soumis à décision, sans convertir RSS en mémoire privée ou ignorer un helper.

P0/P1 de lisibilité/accessibilité basique terminés ; raffinements facultatifs P2 ne retardent pas indéfiniment la RC. DS-UI peut corriger textes/alignements après APIs figées ; DS-PERF peut analyser traces déjà anonymisées, jamais choisir seul une modification de confiance. Sol xhigh garde la décision sécurité/performance et la validation finale. **Arrêt avec bilan mesuré avant étape5.**

## 5. Étape5 — release candidate et qualification

Créer une branche RC0.22 isolée depuis le code intégré vérifié, pas depuis la candidate native0.21.1. Aucun P0/P1 ouvert ; geler les fonctionnalités et documenter chaque preuve sur le commit exact.

### Checklist d'entrée et de qualification

1. Matrice de parité : chaque P0/P1 satisfait, aucun bouton factice/native-only comme substitut ; P2 restant explicitement hors release. Versions Cargo/Flutter/resources/MSI/app/receipt/docs cohérentes0.22.0.
2. Cinq jobs natifs exact-source : Rust fmt/check/build release/tests/Clippy strict ; Flutter analyse/tests/release, packages et architecture/ABI. Job skipped/cancelled ≠ pass ; lire reçus et journaux, pas seulement le badge global.
3. Fenêtres natives réelles dans les environnements autorisés : ouverture par tous les points d'entrée, unicité, close/reopen cycles, panneaux,7langues/2thèmes, erreurs IPC/stale/denied, Rust/notifications actifs après sortie ; captures uniquement des fenêtres possédées. Réutiliser B5 pour méthode, pas comme preuve de nouveaux bytes.
4. Windows standard WinSta0/Explorer jetable et test sans runtime préinstallé ; MSI install/upgrade0.21.0/repair/uninstall, Cargo/portable selon propriétaire, autostart/préférences/history/protection inchangés. Ne pas installer le candidat sur le PC quotidien.
5. Linux Mint x64 réel et Linux ARM64 automatisé : SNI, erreurs/permissions, composantes partagées et renderer exact. Si aucun desktop ARM accessible, l'exécution reste non vérifiée ; ne pas marquer un build runtime. Rechercher un environnement automatisé autorisé avant proposition de RC, sans déclarer du matériel physique inventé.
6. macOS Intel/ARM via runners : AppKit permanent/Flutter/IPC/panneaux/seals/packages/autostart contrôlable, sans exigence de Mac manuel ; limites TCC/capteurs/autorisation matérielle clairement séparées.
7. Performances comparées à la stable : pas de régression importante inexpliquée, ni chiffre d'un prototype ou profil instrumenté requalifié comme release.
8. Paquetage : conserver les familles existantes Windows ZIP/MSI, Linux x64/ARM64 archives, macOS x64/ARM64 archives et DMGs ; chaque conteneur contient le payload hybride réel. Préparer l'inventaire exact (base actuelle dix assets : huit packages, SBOM SPDX, SHA256SUMS), SHA de chaque vrai fichier et fichiers natifs, SBOM incluant Rust/Dart/Flutter/composants natifs/licenses, attestations liées à source/artefact/workflow. Toute variation d'artefact nécessite une revue explicite.
9. Vérifier signatures réellement présentes : Windows signé seulement si certificat valide vérifié sur **tous** les payloads concernés ; sinon NotSigned annoncé. Ad-hoc macOS ≠ Developer ID/notarisation/Gatekeeper. SHA/provenance ≠ autorisation système, SmartScreen ou antivirus.
10. Préparer notes0.22 réellement sourcées, limitations/plateformes, procédure upgrade depuis0.21 et rollback. Préserver privacy/site/SEO et la version publique actuelle jusqu'à publication. Pas de manifeste winget0.22 avant les assets publics.

### Conditions d'arrêt et rollback

- P0/P1 incomplet, résultat de sécurité/ownership/restore incohérent, dépendance inconnue, checksum/source erroné, package manquant ou régression CPU importante inexpliquée => **NO-GO RC**, correction minimale puis reprise des tests affectés.
- Pas de promesse de downgrade MSI : MajorUpgrade refuse l'ancien produit. Rollback de transaction avant commit conserve un ensemble cohérent ; récupération après publication exige un plan approuvé ou une nouvelle version corrective, jamais réécriture de tag/asset.
- Un scénario nécessitant privilège/compte/VM indisponible reste non exécuté ; demander uniquement l'autorisation ou le prérequis réellement nécessaire. Aucun environnement GitHub/desktop/token falsifié pour passer les guards.

Sol xhigh décide des preuves et du GO/NO-GO. DS-TEST peut relier les sorties/manifestes/versions et détecter des gates manquants sur des artefacts figés ; pas d'autorité sur la conclusion de sécurité. **Point d'arrêt obligatoire : GO RC ou NO-GO RC au propriétaire, aucune publication automatique.**

## 6. Étape6 — publication GitHub, site et winget

Dépendance : RC validée, autorisations explicites appropriées. Préparer les commandes et contrôler les automations avant chaque action ; l'accord pour tester/pousser une branche ne vaut pas accord tag/publication.

### Ordre obligatoire

1. Vérifier refs/distances, arbres/worktrees/stash/protected tag, exact source candidate et droits/actions automatiques. Intégration de main uniquement après accord ; conserver privacy/SEO/historiques. Sur le commit intégré, vérifier les contrôles requis et que le site conserve encore la version publique précédente.
2. Faire approuver le commit final à taguer et les avertissements. Le tag `v0.22.0` doit pointer uniquement sur ce SHA ; les tags actuels restent immuables. Examiner le workflow Release et remplacer les notes0.21/0.21.1 et le build natif par les vrais produits0.22. Aucun tag pendant étapes1–5.
3. Après autorisation finale, tag unique, workflow Release ; surveiller tous jobs et reçus, vérifier ensemble complet/architectures/ABI/packages/SHA/signatures/SBOM/attestations source+tag. Ne pas continuer vers site/winget sur un échec.
4. Vérifier release publique stable/latest avec titre **v0.22.0**, notes complètes, limites exactes. En cas de panne GitHub, vérifier release/draft/uploads avant tentative officielle ; réutiliser les artefacts exacts déjà vérifiés, ne pas reconstruire/remplacer silencieusement un package publié.
5. Seulement si release complète : mettre à jour `site/release.json`, liens/versions/checksums sept langues par le processus existant ; préserver privacy et SEO/canonical/hreflang/WebSite/favicon/sitemap. Déployer seulement après accord couvert ; tester les URL réelles sans JS/mobile/desktop et les vrais téléchargements.
6. Puis winget : conserver **RomanCuisset.MicCamWatch**. Récupérer l'URL réelle du MSI0.22 publié, télécharger indépendamment et vérifier SHA contre le manifeste/provenance, inspecter ProductName/Version/Manufacturer/ProductCode/UpgradeCode/scope/registre réel d'une installation. Lire le schéma winget alors courant et comparer le manifeste accepté0.13.4. Ne pas copier le ProductCode aléatoire de la RC locale.
7. Générer les trois manifestes avec `distribution/winget/Generate-Manifests.ps1` et les vraies valeurs. Ne pas ajouter Microsoft.VCRedist par réflexe : la garde et le test propre du bundle complet décident des dépendances réelles. Si une dépendance est nécessaire, justifier son plancher et refaire les contrôles. Valider via les outils/schémas winget officiels, installation/upgrade/removal en guest ; PR de mise à jour seulement après accord, aucun nouvel identifiant.
8. Suivre les contrôles Microsoft et l'index : disponibilité effective/upgrade vers0.22, pas « PR soumise = disponible ». Mettre à jour ROADMAP/release history, publier les liens de preuve et limites sans exposer journaux personnels.

### Winget ancien0.13.4

La PR441153 est fusionnée et l'ancienne version garde son problème de runtime. Aucun asset ancien modifié, PR fermée ou commentaire public envoyé dans cette mission. Si une mesure urgente est demandée avant0.22, option distincte : manifeste de dépendance VC documenté et testé avec minimum réel, après autorisation séparée ; pas prétendre que l'ancien MSI possède le CRT statique. La soumission0.22 attend GitHub et ses vraies URL/SHA.

DS-WINGET peut, dans une session autorisée, préparer/valider les manifestes **après** publication avec valeurs réelles et comparer le registre. Sol xhigh garde l'intégration finale, le commit exact, tag/provenance, conditions d'arrêt et décision publique.

## 7. Protocole reproductible CPU/RAM et démarrage

**Protocole préparé ; aucune nouvelle mesure de performance ou action matérielle dans l'étape1.** Ne pas réutiliser les chiffres B5 comme promesse de0.22.

### 7.1 Admission et baseline

- Télécharger l'immuable release0.21.0 avec manifeste/digests/API vérifiés ; conserver exact SHA/source/type d'install. Candidate release optimisée, non instrumentée, sans debugger/devtools, avec même politique effective, droits, contexte OS et paramètres que la stable. Pas de build modifié0.21 rebaptisé « vraie stable ».
- Windows comparatif natif opérationnel dans une session interactive **jetable** ; jamais le tray quotidien. L'ancien natif a un singleton de session : APPDATA privé ne suffit pas à isoler. `watch --no-kill` peut reprendre une garde demandée ; ne pas le lancer sur l'hôte sous prétexte de lecture seule. Un nouveau compte/VM requiert une autorisation distincte.
- Ne pas comparer un owner opérationnel0.21 à un mode B5 read_only puis appeler la différence une optimisation moteur. Mesurer mêmes modes/fonctions. Les modes privés B5 restent utiles pour profilage différentiel B5→0.22, étiqueté séparément.
- CPU baseline et test « sans VC » sont deux fixtures : installer outils/redistribuables dans la première peut invalider la seconde. Pas de suppression d'un runtime partagé pour fabriquer un Windows propre.
- Garder une seule installation/owner opérationnel par essai, stop normal vérifié entre variantes, jamais kill par nom/PID seul. Aucun guard ayant un état de restauration non résolu. Pas de mutation des périphériques sans accord spécifique et protocole sûr dans l'environnement prévu.

### 7.2 Scénarios et comparabilité

| Scénario | Mesures distinctes | Conditions / quand non mesuré |
|---|---|---|
| Rust/tray seul, UI fermée | CPU, RAM, threads/processus, helpers/gardes, cadence effective | Baseline principale de légèreté ; aucun processus Flutter résiduel |
| Fenêtre native0.21 ouverte vs Flutter0.22 ouverte | Arbre entier, incremental UI, CPU/renderer, réactivité | Ce sont des fenêtres différentes ; ne pas présenter le delta comme coût du moteur Rust seul |
| Fermeture et stabilisation | Même Rust toujours actif, mémoire libérée UI, CPU et fin réelle des enfants | Respecter délais de fermeture ; un PID disparu/réutilisé n'est pas une preuve de fin de la bonne instance |
| Aucune activité observée / Ready caméra | Snapshot de santé/confiance/couverture, processus et politique identiques | Unknown/Unavailable n'est pas un scénario « inactif sain » ; ne pas mélanger Ready et Active |
| Micro actif / caméra active / plusieurs apps | CPU/stades/signatures/latence START-UPDATE-STOP | Seulement capture autorisée ou source virtuelle de qualification ; sinon **non mesuré**. Ne pas actionner une caméra personnelle ou réduire la qualité des preuves |
| Notifications/sons | Surcoût Rust sans UI, nombre de soumissions/erreurs/cooldown | Préférences identiques, TEST signalé comme synthétique et séparé de vraie capture |
| Linux logiciel vs accéléré | RSS/PSS/CPU/latences par renderer et serveur X11/Wayland/session | Vérifier **renderer réellement utilisé par Flutter**, pas seulement `direct rendering: Yes`. llvmpipe/XRDP n'est pas un benchmark GPU ; aucun changement driver/droits |

- Trois paires équilibrées au minimum, ordre AB/BA ou ABBA/BAAB alterné ; warmup30s, intervalle mesuré180s par état. Conserver valeurs par intervalle et moyenne/médiane/min/max/dispersion, pas une seule moyenne choisie. Allonger selon variabilité avant de conclure, pas calendrier de livraison.
- Relever policy/trust, flags/cadence, source/byte hashes, OS/CPU/power plan/toolchain/engine, renderer/session, processus applicatifs et état de protection, santé/confiance/evidence/started_at aux bornes **et transitions pendant l'intervalle**. Les seules bornes égales n'excluent pas une transition intermédiaire.
- Changement de cohorte/état/signature/policy, contention build/update, erreur collecteur ou perte de processus => intervalle invalide conservé et exclu explicitement. Ne normaliser que l'ordre et la version outil ; ne supprimer aucune différence de santé/risque/enforcement/activité pour rendre deux scénarios égaux. Comparaisons entre deux snapshots de VM distincts nécessitent une identité de workload contrôlée explicitement justifiée, pas suppression aveugle des PID.
- Profiler WinVerifyTrust, signer, Toolhelp, modules, sensor, audio, snapshot, IPC/rendu dans une **campagne séparée**. Compter appels et échecs ; scopes imbriqués non additionnables. Coût de cache/invalidation inclus si une optimisation est proposée, sans conserver un verdict périmé.

### 7.3 Mesures et rapport

- CPU = `100 × delta(user+kernel) / durée murale` sur tout l'arbre ;100%=un cœur logique. Donner la normalisation et la fenêtre ; relever enfants éphémères et CPU à leur sortie si possible, sinon annoncer un plancher.
- Windows : private committed et working set RSS séparés ; USS/shared split seulement si réellement disponible. Linux : RSS/PSS/USS via procfs/smaps si autorisé, shared ne pas compter comme privé. macOS : resident/footprint/private selon API disponible, **non mesuré** sinon. Ne pas sommer PSS et RSS ou comparer commit Windows à PSS Linux comme même métrique.
- Réutiliser le suivi d'identité retenue (handle/FILETIME, `/proc` birth/pidfd, libproc), pas des noms de processus. Arbre application inclut gardes/helpers/Flutter, pas seulement parent ; mémoire du serveur graphique hors arbre signalée séparément.
- Timestamps parent monotoniques : création processus, moteur créé, Dart démarré, snapshot reçu, première frame, Ready protocole, **première fenêtre réellement visible/utilisable**, demande close, fin noyau, retour mémoire stabilisé. Ne pas soustraire horloges child/parent non corrélées.
- Distinguer première ouverture observée, cache ordinaire et vraie ouverture disque-froid uniquement si démontrée ; ne pas vider les caches globaux du PC. Six cycles au moins, thèmes alternés, et série longue pour déceler fuite/instances multiples. Aucun préchauffage résidant Flutter.
- Latence de détection : référence contrôlée monotone de début/fin workload dans fixture autorisée, puis observation native et rendu ; garder la cadence réelle et distribuer les répétitions sur les phases du poll. Ready de protocole n'est pas une latence de détection d'un capteur.
- Tailles : fichiers réels distribués, conteneur compressé et payload extrait séparés ; signatures/attestations sur les bytes finaux. Pas de chiffres de prototype ni de fichiers doublement comptés.
- Budget historique15MB conservé comme objectif non universellement atteint ; coût UI ouverte documenté séparément. Pas de hausse de seuil de test pour favoriser Flutter. Toute régression importante, inconnue Active caméra ou absence de scénario obligatoire est un gate à résoudre/décider, pas un résultat vert implicite.

### 7.4 Résultats historiques à ne pas mélanger

- B5 Windows comparé **à B4**, pas à0.21 : CPU fermé36.983→35.910%, ouvert32.562→34.157%, RSS B5 fermé20.210/open136.504Mio ; pas de gain général. Profil séparé WinVerifyTrust33.16% d'un cœur ; pas à additionner au snapshot inclusif.
- Mint B5 XRDP/llvmpipe : open RSS324.72–324.79Mio/PSS295.47–295.50Mio, fermé10.48–10.77Mio ; première fenêtre4.83–5.68s, reopens environ1s. Pas GPU accéléré.
- RC native0.21.1 vs vraie0.21 : CLI750ms, quatre180s, cohort micro naturellement Active/high+deux caméras Ready/low ; CPU55.4545/55.3532%, RSS25.7013/25.3503Mio. Ce n'est ni tray, ni Flutter, ni0.22. Première séquence caméra Active non concluante et conservée ; ne pas affirmer ce chemin résolu.

## 8. Répartition proposée entre modèles

**Aucune délégation n'est lancée maintenant.** Sous-agent futur seulement si même Sol/Codex OAuth effectivement prouvé ; DeepSeek peut intervenir uniquement comme agent principal d'une autre session autorisée, sur branche/worktree dédié sans fichiers partagés en cours.

| Identifiant / niveau recommandé | Périmètre indépendant autorisable | Entrée / sortie / limites |
|---|---|---|
| SOL-ARCH — Sol xhigh | Admission IPC/contrats/capacités, restauration micro/caméra, confirmations, politique Kill, parent/child ownership | Obligatoire dans ce plan pour les décisions P0, leur intégration et revue ; pas une affirmation qu'un autre modèle serait intrinsèquement incapable |
| SOL-PACK — Sol xhigh | Reçus/version/bundle trust, transactions update/rollback/uninstall, changement ABI ou identité Mac, validation finale | Arbitre les dépendances communes aux5cibles ; inspecte toute proposition DS avant intégration |
| SOL-RELEASE — Sol xhigh | Exact-source RC, comparaison CPU/sécurité, GO/NO-GO et opérations publiques autorisées | Aucun décalage de tag, remplacement asset ou relèvement de seuil silencieux |
| DS-UI — DeepSeek HIGH, session séparée | Labels/traductions About, composants layout/semantics sur DTO déjà figés | Fichiers Dart de présentation assignés ; pas commands/model/security simultanément ; tests de comportement/layout, pas réécriture moteur |
| DS-TEST — DeepSeek HIGH, session séparée | Tests consommateurs de préférence/history/diagnostics et revue de reçus après contrats | Corpus déterministe privé ; pas mock echo ni assertions source-text ; ne change pas un guard pour passer |
| DS-PACK — DeepSeek HIGH, session séparée | Inventaire payload/licenses/versions, tests de présence/architecture/manifestes selon schéma figé | Sources de packaging assignées seulement après layout commun ; aucune décision de rollback/autorité, pas de modification hôte |
| DS-PERF — DeepSeek HIGH, session séparée | Analyse de traces anonymisées, tableaux/dispersion/cohortes et diagnostic des différences | Pas d'optimisation trust ou cadence décidée seul ; résultats inconclusifs conservés |
| DS-WINGET — DeepSeek HIGH, session séparée | Après release : vrais manifestes, schémas/registre/URLs/SHA et préparation PR | Valeurs publiques réelles uniquement ; pas de soumission/commentaire sans accord ; même identifiant |

Ne pas sélectionner un fournisseur différent à partir de cette session. Le mot HIGH est un niveau recommandé à vérifier dans l'environnement DeepSeek, pas une attestation d'une configuration actuelle. L'intégration, les tests finaux et l'autorité Git restent à un propriétaire ; périmètres indépendants avant fan-out, pas deux agents sur les mêmes fichiers.

## 9. Conditions définitives de publication0.22

- Tous P0/P1 de la matrice sont implémentés et leurs gates appropriés réellement satisfaits ; pas de migration déclarée complète grâce au secours natif.
- Cinq distributions hybrides exact-source, ABI/minimums inchangés ou exception explicitement approuvée ; vérifications build/tests/UI/IPC/installed-package distinguées de matériel non testé.
- Windows runtime-free/standard/upgrade, restitution/permissions/pending, version/update/uninstall et un seul produit réellement qualifiés dans environnements adaptés.
- Pas de régression sécurité ou CPU importante inexpliquée ; objectifs de légèreté inchangés, moteur graphique absent après close, aucune surveillance affaiblie.
- Preuves macOS exclusivement automatisées et limites TCC/matériel documentées ; aucun Mac manuel à organiser. ARM physique non prétendu validé ; capacité/source/build/runtime/packaging explicitement distingués.
- Artefacts/SHA/SBOM/provenance/signatures réels, notes honnêtes, rollback et conditions d'arrêt relus, autorisation finale du propriétaire. Stable0.21.0, tags, données et protections préservés jusqu'aux opérations autorisées.
- GitHub stable complète avant site/winget ; aucun manifeste URL/SHA fictif, aucun nouvel identifiant winget.

**Statut actuel : ÉTAPE1/6 uniquement.** Les références, la matrice et ce plan sont livrés ; étapes2–6 non commencées. Prochaine décision demandée après lecture du rapport : valider le périmètre et autoriser **étape2 avec GPT-6.1 Sol xhigh**, pas encore push, installation, mutation matérielle ou publication.
